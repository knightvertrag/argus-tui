# MEMORY — argus-tui (LLM pickup)

Dense project context. Read this before architecture/session/DAP docs. Do not invent layers that are not here.

**Updated:** 2026-09-25
**Crate:** binary only (`src/main.rs`). No `lib.rs`. Edition 2024.

---

## What this is

Rust TUI debugger for **C**, frontend over **`lldb-dap`** via the Debug Adapter Protocol. Not an LLDB CLI wrapper. Long-term: same session driven by TUI or an agent.

**Now:** ratatui UI on `debugger::Session`. `cargo run -- <program>` is the TUI. `--harness` prints one stop. Source breakpoints, watches (`evaluate`), and registers work.
**Not now:** attach, disassembly, memory, expanding structured variables, launch-config files, concurrent DAP requests, reverse-request replies.

---

## Layers (do not skip)

```
TUI draw loop          ui/* paints ViewModel only
    ↓ commands
driver                 owns Session; publishes ViewModel
    ↓
debugger::Session      handshake, Phase, inspect/step, domain events
    ↓
dap::Client            spawn, seq, one in-flight request, frame-reader task
    ↓
dap::types + framing   JSON Message / Request trait / Content-Length
    ↓
xcrun lldb-dap         stdin/stdout piped; stderr read and traced
```

**Contract:** Client emits typed DAP. Session owns meaning. UI owns pixels.
TUI/harness/agent call `Session`, never `framing` or `msg["command"]`.
Session is the only intended `Client` consumer. The driver is the TUI's only `Session` caller.

---

## Tree

```
src/main.rs              args; TUI or --harness
src/driver.rs            Session owner; Command → ViewModel watch channel
src/view.rs              ViewModel snapshot (no Session calls from widgets)
src/command.rs           prompt parser
src/highlight.rs         hand-rolled C line highlighter
src/event.rs             crossterm stream (no tick)
src/logging.rs           stderr (harness) or argus-tui.log (TUI)
src/ui/                  layout, theme, panes, keys, help
src/dap/                 framing, Client, types (includes Evaluate)
src/debugger/            Session, Phase, SourceCache
docs/                    architecture, session, dap, roadmap
screens/tui_proto.jpg    visual target for the Code view
testdata/hello.c + hello (-g -O0)
```

Code view (`screens/tui_dense.jpg`): file bar, then left Breakpoints / Watch / Call Stack, center Code, right Variables / Register. Tabs: Code, Vars, Stack, Threads, Memory, Terminal. Prompt stays. Memory has no data yet.

---

## Decisions (do not reverse without asking)

1. Hand-written DAP subset. No `dap` / `dap-types` crates. Schema codegen later.
2. `Request` trait + uninhabited markers (`enum Initialize {}`), not a giant Command enum (that is for servers).
3. Two-step decode: `Message` envelope (untyped command/event + `Value` body) then `decode_success::<R>()` / `EventMessage::parse()`.
4. Unknown events → `Event::Unknown`. Extra initialize flags → `Capabilities.extra` flatten.
5. One in-flight request (`&mut Client`). Events during wait go to inbox; Session `apply_inbox` after every `request()`.
6. Do not advertise `supportsRunInTerminalRequest` / `supportsStartDebuggingRequest`. Reverse requests are queued, not answered.
7. `thiserror` in `dap`/`debugger`. `anyhow` in `main`. No `color_eyre`.
8. `Seq` and DAP ids are `i64`. Envelope field is `request_seq` (underscore), not camelCase.
9. `adapterID` / `clientID` serde rename `ID`. `Variable.type_field` → JSON `"type"`.
10. Spawn is `xcrun lldb-dap` (macOS). Logging via `tracing`; client does not `println`. TUI log file is `argus-tui.log`. Adapter stderr is piped and traced at debug (`target: lldb_dap`).
11. Docs split: architecture = integration/flow; session-implementation / dap-implementation = internals.
12. `framing::read` is not cancellation-safe. A Client task is the only reader and emits complete `Message`s. `recv` may be cancelled; `request()` must run to completion.
13. Do not `canonicalize` breakpoint paths. On a case-insensitive volume that rewrites `COde` → `Code` and lldb will not verify the breakpoint. Send the path the user typed.

---

## Session API (what to call)

```
Session::launch(LaunchConfig)     spawn, initialize, launch, optional bps, configurationDone
wait_until_stopped()              harness: wait Stopped (or Ended); then threads + stackTrace
next_event()                      driver loop
resume / step_over / step_in / step_out     require Stopped; enter_running then DAP
set_breakpoints(path, &[BreakpointSpec])    line + optional condition; replaces the file's set
refresh_stop_context / select_thread / select_frame
load_frame_variables()            locals + Registers scope (expanded one level)
load_locals()                     same, returns the locals slice
evaluate(expr)                    DAP evaluate, context "watch", focused frame
disconnect()                      terminateDebuggee; always shutdown
state() / sources()
```

`LaunchConfig::new(program)` has `stop_on_entry: false`. Harness sets `true`. TUI flag `--stop-on-entry`.
`LaunchConfig.breakpoints` is still `(path, lines)`; launch maps them to specs with `condition: None`.

`Phase`: Starting | Running | Stopped | Exited | Terminated.
Inspect/step require `Stopped`. `enter_running()` clears frames, locals, and registers; no-op if already Exited/Terminated.

`SessionState.breakpoints`: `BTreeMap<PathBuf, Vec<BoundBreakpoint>>` — spec (line, condition) plus adapter `Breakpoint`. The adapter does not echo the condition.
`SessionState.registers`: variables from the scope named `Registers`. lldb nests `x0`/`rax` under groups; the session expands one level.

`SessionEvent` wraps DAP payloads (`Stopped(StoppedEvent)`, …). Unknown → `Adapter { event }`.

Watches live on the driver / `ViewModel`, not in `SessionState`.

---

## DAP types worth remembering

Markers: Initialize, Launch, ConfigurationDone, SetBreakpoints, Threads, StackTrace, Scopes, Variables, Evaluate, Continue, Next, StepIn, StepOut, Disconnect.

`LaunchArguments`: lldb-dap `program`/`args`/`cwd`/`env`/`stopOnEntry` + flatten `extra`.
`InitializeArguments::new("lldb-dap")` sets clientID argus-tui, pathFormat path, 1-based lines/cols, supportsVariableType.
Empty args/`()` omit `arguments` on the wire. Success body `()` accepts missing/`{}`.

Framing: `Content-Length: N\r\n\r\n` + UTF-8 body; N is **bytes**. Caps 1 MiB headers / 32 MiB body.

---

## Gotchas

- `--stop-on-entry` is a function breakpoint on `main`, not DAP `stopOnEntry`. The DAP flag stops in `_dyld_start` (`module`symbol` paths are not files). A loader frame still shows `Stopped in {name} (no source)`.
- Source breakpoint `-b testdata/hello.c:14` **does** stop in `main` (reason `Breakpoint`) when the path string matches DWARF. Canonicalizing the path made it unverified on this machine.
- arm64 lldb register names are `x0`, `x1`, … The prototype image showed x86 `rax`.
- `Event` Serialize is the Rust enum shape, not DAP wire (`EventMessage` is the wire).
- `Client::recv` unexpected response uses `expected: 0` (no pending request).
- Do not `select!` around `request()`. Cancelling it drops the matching response on the floor. The driver only selects between commands and `next_event()`.
- After `resume`/`step_*` returns, a `stopped` may already have been applied from the inbox. The driver inspects immediately when phase is `Stopped`.

---

## Commands

```
clang -g -O0 -o testdata/hello testdata/hello.c
cargo test
cargo clippy --all-targets -- -D warnings
cargo run -- -b testdata/hello.c:14 testdata/hello
cargo run -- --harness
RUST_LOG=argus_tui=debug cargo run -- --harness
```

No unit test spawns the adapter. UI tests use `TestBackend` and a fixture `ViewModel`.

---

## Next

Phase 3 leftovers: variable children, launch-config files, source path mapping, richer terminal filtering. Attach, disassembly, and memory are still out. The driver `Command` enum is the in-process surface for a future agent; there is no socket or MCP server.

---

## Detail docs

- `docs/architecture.md` — layers and sequence diagrams
- `docs/session-implementation.md` — handshake, inbox, apply_event
- `docs/dap-implementation.md` — framing, serde, Client
- `docs/tui/README.md` — TUI implementation tree (driver, screen, input)
- `docs/tui-c-debugger-plan.md` — roadmap; phases 0–2 are done
