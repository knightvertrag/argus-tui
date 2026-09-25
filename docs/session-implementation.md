# Session implementation notes

Low-level description of `debugger::Session`. For layering and integration, see [`architecture.md`](./architecture.md). The DAP client it sits on is documented in [`dap-implementation.md`](./dap-implementation.md).

---

## Layout

```
src/debugger/
  mod.rs      Session, LaunchConfig, SessionEvent, SessionError
  state.rs    Phase, SessionState, apply_event
  source.rs   SourceCache (disk files, 1-based lines)
```

`Session` owns a `dap::Client`, `SessionState`, and `SourceCache`. Nothing in this module parses `Content-Length` or builds raw JSON.

Logging: INFO for launch and phase changes (Stopped/Running/Exited/Terminated); DEBUG for public commands, `apply_event` phase transitions, and unknown adapter events (`module`, `capabilities`, …); WARN for reverse requests and unverified breakpoints. Wire JSON is logged on the client, not here.

---

## Public types

### `LaunchConfig`

| Field | Meaning |
|-------|---------|
| `program` | Path to the debuggee |
| `args` | argv after `program` |
| `cwd` | Optional working directory |
| `stop_on_entry` | DAP `stopOnEntry` |
| `breakpoints` | `(source path, lines)` applied **after** `initialized` and **before** `configurationDone`. Lines become `BreakpointSpec`s with no condition. The TUI does not canonicalize these paths. |

`LaunchConfig::new(program)` leaves `stop_on_entry` false.

### `Phase`

`Starting` → `Running` | `Stopped` | `Exited` | `Terminated`.

Stepping and inspect APIs require `Stopped`. `enter_running()` is a no-op if the process already exited or terminated.

### `SessionState`

Read via `session.state()`:

- `capabilities` from `initialize`
- `threads` / `focused_thread`
- `frames` / `focused_frame`
- `scopes` / `locals` / `registers`
- `stopped: Option<StoppedEvent>`
- `process`, `exit_code`, `output`
- `breakpoints: BTreeMap<PathBuf, Vec<BoundBreakpoint>>` (`BreakpointSpec` plus the adapter `Breakpoint`; the condition lives only on the spec)

`location()` is the focused frame’s function, path, line, column (DAP 1-based).

### `SessionEvent`

Wraps DAP event payloads rather than replacing them: `Stopped(StoppedEvent)`, `Output(OutputEvent)`, … Unknown DAP events become `Adapter { event }`. Reverse requests become `ReverseRequest { command }` and are not answered.

### `SessionError`

`Client` (transparent), `Ended` (exited/terminated while waiting for a stop), `WrongPhase`, `NoThread`, `NoFrame`, `Source` (I/O).

---

## Launch handshake (`Session::launch`)

1. `Client::spawn()` (`xcrun lldb-dap`).
2. `initialize` with `InitializeArguments::new("lldb-dap")`; store `Capabilities`.
3. `launch` (`program`, `args`, `cwd`, `stopOnEntry`). Do **not** wait for `initialized` first.
4. `wait_for_initialized()` — `next_event()` until `SessionEvent::Initialized` (`Ended` if the debuggee dies).
5. `set_breakpoints` for each entry in `LaunchConfig.breakpoints`.
6. `configurationDone` if `supports_configuration_done_request`.
7. If phase is not already `Stopped` / `Exited` / `Terminated`, set `Running`.

Every `request()` drains the client inbox afterward so `process` / `thread` / `stopped` that raced the response still update state.

---

## Event application

```
Incoming::Event(e)  →  SessionState::apply_event(&e)  →  SessionEvent
Incoming::ReverseRequest  →  SessionEvent::ReverseRequest  (state unchanged)
```

`apply_event` (in `state.rs`):

| DAP event | State change |
|-----------|----------------|
| `initialized` | none (handshake watches the `SessionEvent`) |
| `stopped` | `Phase::Stopped`, `focused_thread`, store `StoppedEvent` |
| `continued` | `enter_running()` if all threads continued, or the focused thread did |
| `exited` | `Phase::Exited`, `exit_code` |
| `terminated` | `Phase::Terminated` |
| `thread` | add placeholder name, or remove on `exited` |
| `output` | append |
| `process` | replace |
| `breakpoint` | replace matching `id` in the breakpoint map |
| `Unknown` | ignore |

`enter_running()` clears `stopped`, frames, focused frame, scopes, locals, and registers (stack is stale while running).

---

## Inbox vs `next_event`

`Client::request` queues events that arrive before the matching response. Session wraps every DAP call:

```rust
async fn request<R: Request>(...) {
    let response = self.client.request::<R>(args).await?;
    self.apply_inbox();  // Client::drain_inbox(), handle_incoming each
    Ok(response)
}
```

`next_event()` is `client.recv()` then `handle_incoming`. Inbox items surface here if nothing drained them.

`wait_until_stopped()`:

- If already `Stopped` (e.g. stop arrived during `configurationDone`), skip the wait.
- Else loop `next_event` until `Stopped`, or `Ended` on `Exited`/`Terminated`.
- Then `refresh_stop_context()`.

---

## Control and inspect

| Method | DAP | Notes |
|--------|-----|--------|
| `resume` | `continue` | focused thread |
| `step_over` | `next` | |
| `step_in` | `stepIn` | |
| `step_out` | `stepOut` | |
| `set_breakpoints(path, specs)` | `setBreakpoints` | `BreakpointSpec` is line + optional `condition`. Replaces that file's set. Stores `BoundBreakpoint` (spec kept; adapter result may be unverified). |
| `refresh_stop_context` | `threads` + `stackTrace` | first 20 frames; focuses first frame; clears scopes/locals/registers |
| `select_thread` | then `refresh_stop_context` | id must already be in `threads` |
| `select_frame` | — | must be in current `frames`; clears scopes/locals/registers |
| `load_frame_variables` | `scopes` + `variables` | non-expensive scopes except `Registers` → `locals`. Scope named `Registers` is loaded even when `expensive`. Its groups (`General Purpose Registers`, …) are expanded one level so the list is `x0` / `rax`, not the group name. |
| `load_locals` | calls `load_frame_variables` | returns the locals slice (harness) |
| `evaluate(expr)` | `evaluate` | `context: "watch"`, focused frame. Returns the `result` string. Watches themselves are not session state. |
| `disconnect` | `disconnect` + `terminateDebuggee` | ignores DAP error; always `client.shutdown()` |

Stepping goes through `step_request`: require `Stopped`, **`enter_running()` first**, then send the DAP request. If a `stopped` is already in the inbox when the response returns, `apply_inbox` overwrites `Running`. If the request fails, phase may stay `Running` — that is accepted on the error path.

`refresh_stop_context` / `load_frame_variables` / `evaluate` / `select_thread` / `select_frame` / stepping all `require_phase(Stopped)`.

---

## Source cache

`SourceCache` reads files from disk once, splits on `\n`. `line(path, n)` is **1-based** (DAP). Out of range → `Ok(None)`. Missing file → `io::Error`.

The harness prints one line via `session.sources().line`. The TUI loads the whole file through `sources().load` when building a `ViewModel`. Neither uses DAP `source` / `sourceReference`.

---

## Harness and TUI (`src/main.rs`)

```
LaunchConfig { program: testdata/hello, stop_on_entry: true }
Session::launch
wait_until_stopped
print phase, reason, thread, stack
print source line for location()
load_locals, print
disconnect
```

On Apple `lldb-dap`, the first `stopOnEntry` stop is often `_dyld_start` with reason `Exception`, not `hello.c`. That is adapter behavior. Register groups from that stop are no longer mixed into `locals`; they land in `registers` after one level of expansion.

`cargo run -- -b testdata/hello.c:14 testdata/hello` has been observed to stop with reason `Breakpoint` in `main`, as long as the path string is the one in DWARF. `canonicalize` on this machine rewrote `COde` to `Code` and the breakpoint stayed unverified. The TUI therefore keeps the path the user passed.

---

## Tests

No session test spawns `lldb-dap`.

- `state.rs`: stopped sets phase/thread; continued clears stack; `location()` from focused frame; `enter_running` does not override `Exited`; a breakpoint event updates `verified` and keeps the spec condition.
- `source.rs`: temp file, 1-based lines, EOF → `None`.

UI tests (highlight, prompt parser, `TestBackend` render, arg parsing) also avoid the adapter. End-to-end is `cargo run -- -b …` or `--harness`.

---

## Sharp edges

- One in-flight DAP request (`Client` is `&mut self`). Session does not overlap calls.
- Reverse requests are not answered. We still do not advertise `supportsRunInTerminalRequest`.
- `wait_until_stopped` assumes `Phase::Stopped` implies `state.stopped` is `Some` (true if the phase was set only via `apply_event`).
- `load_frame_variables` skips other `expensive` scopes. Only the scope named exactly `Registers` is loaded when expensive.
- The driver must not cancel `request()`. After a step returns, check phase: inbox may already have applied `Stopped`.
