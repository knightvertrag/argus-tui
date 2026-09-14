# Architecture

How the pieces fit, and how a command flows from a future TUI (or agent) down to `lldb-dap`.

| Doc | Contents |
|-----|----------|
| This file | Layers, integration contract, launch/debug flow |
| [`session-implementation.md`](./session-implementation.md) | `debugger::Session` internals |
| [`dap-implementation.md`](./dap-implementation.md) | Framing, JSON types, `dap::Client` |
| [`tui-c-debugger-plan.md`](./tui-c-debugger-plan.md) | Roadmap |

**Status:** DAP client and debug session exist. `cargo run` is a smoke harness on `debugger::Session`. The TUI is still the ratatui template.

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
│           TUI (ratatui)             │  not wired — template App remains
├─────────────────────────────────────┤
│     Debug Session / State Machine   │  implemented — debugger::Session
├─────────────────────────────────────┤
│      DAP Client (lldb-dap)          │  implemented — framing + types + Client
└─────────────────────────────────────┘
```

Today’s consumer of the session is `src/main.rs` (harness), not the TUI. That is the integration test for the middle layer.

```
harness / TUI / agent
        │  LaunchConfig, resume, step_*, load_locals, state()
        ▼
debugger::Session          phase, threads, frames, locals
        │  request::<Initialize>, recv, drain_inbox
        ▼
dap::Client                seq, one in-flight request, event inbox
        │  Content-Length frames + JSON Message
        ▼
lldb-dap stdin/stdout
```

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

The harness uses `stop_on_entry: true` so it gets a stop without breakpoints. Apple `lldb-dap` often reports that first stop as a loader `exception` in dyld, not `entry` in `hello.c`.

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

Callers that want a live event loop (TUI) use `next_event()` instead of `wait_until_stopped()`, then `refresh_stop_context()` / `load_locals()` when they see `SessionEvent::Stopped`.

---

## What each side of the session boundary looks like

**Into the session** (stable for TUI and agents):

- `Session::launch` / `disconnect`
- `resume`, `step_over`, `step_in`, `step_out`
- `set_breakpoints`, `select_frame`, `load_locals`
- `wait_until_stopped` or `next_event`
- `state()` / `sources()`

**Out of the session:** `SessionState` (phase + inspect snapshots) and `SessionEvent` (DAP payloads wrapped, not re-invented).

**Below the session:** `dap::Client::request::<R>()` and `Incoming::{Event, ReverseRequest}`. The session is the only module that should call those.

---

## Not built

- TUI panes and keybindings (`src/ui/`, `src/app.rs` still template)
- Concurrent in-flight DAP requests
- Reverse requests (`runInTerminal`) — not advertised, not answered
- Attach, evaluate, disassemble, memory

Logs go to **stderr** (`tracing`). Default `argus_tui=info`; `RUST_LOG=argus_tui=debug` for DAP command/event names, `trace` for JSON bodies. See the DAP and session implementation docs.

---

## Run

```bash
clang -g -O0 -o testdata/hello testdata/hello.c
cargo test          # framing, types, session state/source (no lldb-dap)
cargo run           # harness via Session (needs xcrun lldb-dap)
```
