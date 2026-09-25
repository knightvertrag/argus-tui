# Architecture

How the pieces fit, and how a command flows from the TUI (or a future agent) down to `lldb-dap`.

| Doc | Contents |
|-----|----------|
| [`../MEMORY.md`](../MEMORY.md) | LLM pickup — decisions, APIs, gotchas |
| This file | Layers, integration contract, launch/debug flow |
| [`session-implementation.md`](./session-implementation.md) | `debugger::Session` internals |
| [`dap-implementation.md`](./dap-implementation.md) | Framing, JSON types, `dap::Client` |
| [`tui/README.md`](./tui/README.md) | TUI implementation: driver, screen, input |
| [`tui-c-debugger-plan.md`](./tui-c-debugger-plan.md) | Roadmap |

**Status:** The TUI is wired. `cargo run -- <program>` draws the debugger. `cargo run -- --harness` is the smoke printer. DAP client and `debugger::Session` sit under a driver that owns the session.

---

## Idea

argus-tui is a terminal debugger for C. The backend is not LLDB’s CLI; it is the [Debug Adapter Protocol](https://microsoft.github.io/debug-adapter-protocol/) spoken by `lldb-dap`.

- One protocol, so the UI is not tied to LLDB’s command language.
- A session layer, so a human TUI and an agent can issue the same commands against the same state.
- A small hand-written DAP client, so we understand the wire before generating types from the schema.

---

## Layers

```
┌─────────────────────────────────────┐
│     TUI (ratatui) + driver          │  ui paints ViewModel; driver owns Session
├─────────────────────────────────────┤
│     Debug Session / State Machine   │  debugger::Session
├─────────────────────────────────────┤
│      DAP Client (lldb-dap)          │  framing + types + Client
└─────────────────────────────────────┘
```

```
keys / prompt
        │  Command
        ▼
driver                     owns Session, publishes ViewModel
        │  LaunchConfig, resume, step_*, set_breakpoints, evaluate, …
        ▼
debugger::Session          phase, threads, frames, locals, registers, breakpoints
        │  request::<Initialize>, recv, drain_inbox
        ▼
dap::Client                seq, one in-flight request, frame-reader task
        │  Content-Length frames + JSON Message
        ▼
lldb-dap stdin/stdout
```

The draw loop never calls `Session`. Widgets read a `ViewModel` snapshot. `--harness` calls `Session` directly and does not draw.

| Layer | Owns | Must not |
|-------|------|----------|
| **TUI** | layout, keys, what to paint from `SessionState` | DAP JSON, process I/O |
| **Session** | handshake, phase, inspect/step commands, domain events | `Content-Length`, `seq` |
| **Client** | adapter process, framing, typed request/response | “what stopped means” |

Contract: **client emits typed DAP; session owns meaning; UI owns pixels.**

---

## Flow: start a debuggee

Session hides the DAP startup order. Callers only pass a `LaunchConfig` and wait for a stop:

```
caller                         Session                         lldb-dap
  │                               │                               │
  │── launch(config) ────────────►│                               │
  │                               │── initialize ────────────────►│
  │                               │◄─ initialize (capabilities) ──│
  │                               │── launch ────────────────────►│
  │                               │◄─ initialized event ──────────│
  │                               │◄─ launch response ────────────│
  │                               │── setBreakpoints? ───────────►│
  │                               │── configurationDone ─────────►│
  │◄─ Session ────────────────────│                               │
  │── wait_until_stopped() ──────►│◄─ stopped / process / … ──────│
  │◄─ Stopped + stack ────────────│                               │
```

Rules the session encodes (callers should not reimplement them):

1. Nothing else until `initialize` returns.
2. `launch` is sent without waiting for `initialized`; both the response and that event are required before configuration.
3. Breakpoints in `LaunchConfig` are set in that window, then `configurationDone`.
4. `wait_until_stopped` is a separate call so the caller can still insert work (or just inspect) after launch.

The harness uses `stop_on_entry: true` so it gets a stop without breakpoints. Apple `lldb-dap` often reports that first stop as a loader `exception` in dyld, not `entry` in `hello.c`. The TUI leaves `stop_on_entry` off unless `--stop-on-entry` is passed, and takes breakpoints with `-b file:line` (path text kept as typed; see MEMORY gotchas).

---

## Flow: a command while stopped

Example: step over, then show locals.

```
TUI/agent                      Session                         Client
  │                               │                               │
  │── step_over() ───────────────►│  phase must be Stopped        │
  │                               │── next { threadId } ─────────►│
  │◄─ ok (now Running) ───────────│                               │
  │                               │                               │
  │── wait_until_stopped() ──────►│  drain events                 │
  │                               │◄─ stopped ────────────────────│
  │                               │── threads + stackTrace ──────►│
  │◄─ Stopped + frames ───────────│                               │
  │── load_locals() ─────────────►│── scopes + variables ────────►│
  │◄─ state().locals ─────────────│                               │
```

The session is sequential: one DAP request at a time. Events that arrive while a request is in flight are queued on the client and applied to `SessionState` before the command returns, so `stopped` cannot be lost behind a `continue` response.

The driver uses `next_event()` instead of `wait_until_stopped()`. On `Stopped` it calls `refresh_stop_context`, `load_frame_variables`, and `evaluate` for each watch. It also inspects if phase is already `Stopped` when a step call returns, because that stop may have been applied from the inbox. `request()` is awaited to completion. `next_event` may be dropped when a key arrives; the frame reader makes that safe. See [`dap-implementation.md`](./dap-implementation.md).

---

## What each side of the session boundary looks like

**Into the session** (stable for TUI and agents):

- `Session::launch` / `disconnect`
- `resume`, `step_over`, `step_in`, `step_out`
- `set_breakpoints` (`BreakpointSpec`: line + optional condition), `select_thread`, `select_frame`
- `load_frame_variables` / `load_locals`, `evaluate`
- `wait_until_stopped` (harness) or `next_event` (driver)
- `state()` / `sources()`

**Out of the session:** `SessionState` (phase + inspect snapshots) and `SessionEvent` (DAP payloads wrapped, not re-invented).

**Below the session:** `dap::Client::request::<R>()` and `Incoming::{Event, ReverseRequest}`. The session is the only module that should call those.

---

## Not built

- Concurrent in-flight DAP requests
- Reverse requests (`runInTerminal`) — not advertised, not answered
- Attach, disassembly, memory, expanding structured variable children
- Launch-config files and source path rewriting
- An agent transport (the driver `Command` channel is in-process only)

Logs: the TUI writes `tracing` to **`argus-tui.log`**. The harness writes it to **stderr**. Default `argus_tui=info`; `RUST_LOG=argus_tui=debug` for DAP command/event names, `trace` for JSON bodies. Adapter stderr is not inherited; the client traces it at debug (`lldb_dap`).

---

## Run

```bash
clang -g -O0 -o testdata/hello testdata/hello.c
cargo test          # framing, types, session, highlight, commands, UI (no lldb-dap)
cargo run -- -b testdata/hello.c:14 testdata/hello
cargo run -- --harness
```
