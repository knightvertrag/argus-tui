# Architecture — DAP layer as built

High-level view of what exists today, how the pieces fit, and how later layers (session, TUI, agents) are meant to plug in.

The roadmap lives in [`tui-c-debugger-plan.md`](./tui-c-debugger-plan.md). Implementation details of the current DAP stack are in [`dap-implementation.md`](./dap-implementation.md).

**Status:** Phase 1 of the DAP client is partially complete. We can spawn `lldb-dap`, exchange framed typed messages, launch a C program, and wait until it stops. Stack/locals inspection and the TUI are not wired yet.

---

## What we are building

argus-tui is a terminal debugger frontend for C. The backend is not LLDB’s CLI; it is the [Debug Adapter Protocol](https://microsoft.github.io/debug-adapter-protocol/) spoken by `lldb-dap`.

That split is the whole product idea:

- One protocol, so the UI is not tied to LLDB’s command language.
- A pure session layer later, so a human TUI and an agent can drive the same debugger.
- A small, hand-written client now, so we understand every byte on the wire before generating types from the schema.

---

## Layers (target vs today)

Target, from the project plan:

```
┌─────────────────────────────────────┐
│           TUI (ratatui)             │  presentation + input
├─────────────────────────────────────┤
│     Debug Session / State Machine   │  domain: stopped, frames, bps
├─────────────────────────────────────┤
│      DAP Client (lldb-dap)          │  protocol + process I/O     ← implemented
└─────────────────────────────────────┘
```

Today only the bottom layer is real. The ratatui `App` in `src/app.rs` is still the template counter. `src/debugger/` is empty. `src/main.rs` is a **smoke harness** that plays the role of a future session: it issues typed requests and waits for typed events.

That is deliberate. The harness is the first integration test of the client, not the product UI.

---

## DAP stack (implemented)

```
src/main.rs          smoke harness (current consumer)
        │
        ▼
src/dap/client.rs    spawn lldb-dap, seq, request/response, event inbox
        │
        ▼
src/dap/types/       JSON shapes: Message envelope + Request trait + Event
        │
        ▼
src/dap/framing.rs   Content-Length: N\r\n\r\n + UTF-8 body
        │
        ▼
lldb-dap stdin/stdout
```

Each box has one job:

| Component | Responsibility | Does not do |
|-----------|----------------|-------------|
| **Framing** | Turn a byte stream into message bodies and back | JSON, commands, seq |
| **Types** | Serde models for DAP JSON | I/O, process lifetime |
| **Client** | Own the adapter process; send one typed request at a time; queue events | Session state, TUI |
| **Harness** | Drive initialize → launch → configurationDone → stopped → disconnect | Interactive debugging |

Nothing above the client should parse `Content-Length` or poke `msg["command"]`.

---

## Two decode steps

A DAP message on the wire is always:

```
{ "seq": …, "type": "request" | "response" | "event", … }
```

That is not enough to type the payload:

- A **response body** depends on which request `request_seq` belongs to.
- An **event body** depends on the `event` name.

So we decode twice:

1. **Envelope** (`types::Message`) — always succeeds for valid DAP, including unknown events and reverse requests.
2. **Payload** — `ResponseMessage::decode_success::<R>()` after correlating `seq`, or `EventMessage::parse()` from the event name.

Unknown adapter traffic (`module` events, extra capability flags) must not crash the client. Unknown events become `Event::Unknown`. Extra initialize flags land in `Capabilities::extra`.

---

## How a request travels

```
harness:  client.request::<Launch>(args)
client:   seq = next_seq++
          Message::Request { command: "launch", arguments }
framing:  Content-Length: …\r\n\r\n{json}
adapter:  … work …
framing:  next frame
client:   if Event → inbox
          if Response.request_seq == seq → decode Launch body (())
harness:  gets () or ProtocolError::Failed
```

Events that arrive *during* `request()` are not dropped. They sit in `Client.inbox` until the caller `recv()`s. That is how `launch` can return while `initialized` / `process` / `stopped` are still available to the harness.

There is **one in-flight request**. Overlapping requests (a `oneshot` map keyed by seq) are a later client change.

---

## Integration: the launch handshake

DAP’s startup order is easy to get wrong. The harness encodes the sequence we will reuse in the session layer:

```
client                          lldb-dap
  │                                │
  │── initialize ─────────────────►│
  │◄─ initialize response ─────────│   wait for this only
  │── launch ─────────────────────►│   do not wait for initialized first
  │◄─ initialized event ───────────│   may arrive before or after launch response
  │◄─ launch response ─────────────│
  │── configurationDone ──────────►│
  │◄─ configurationDone response ──│
  │◄─ stopped (and others) ────────│
  │── disconnect ─────────────────►│
```

Rules we actually follow:

1. Until `initialize` returns, send nothing else (spec).
2. Send `launch` immediately after; drain until both the launch response **and** the `initialized` event exist.
3. Then `configurationDone`, and wait for its response (the old untyped harness skipped this).
4. Then wait for `stopped`.

`stopOnEntry: true` is how the harness gets a stop without setting breakpoints. On some `lldb-dap` builds the first stop reason is `exception` (loader) rather than `entry`. The client reports whatever the adapter sends.

---

## What the next layers should consume

The session machine should talk to `dap::Client`, not to framing or JSON:

```rust
client.request::<Initialize>(InitializeArguments::new("lldb-dap")).await?;
client.request::<Launch>(launch).await?;
match client.recv().await? {
    Incoming::Event(Event::Stopped(s)) => { /* update domain state */ }
    Incoming::Event(Event::Unknown { event, .. }) => { /* log, ignore */ }
    Incoming::ReverseRequest(_) => { /* not advertised; log */ }
}
```

Domain events (`debugger::Stopped { thread, reason, … }`) should wrap these types, not replace them. The TUI should subscribe to domain state, not to `Message`.

That is the integration contract: **Client emits typed DAP; session owns meaning; UI owns pixels.**

---

## Explicitly not built

- Debug session / breakpoint store / source cache (`src/debugger/`)
- TUI panes (`src/ui/` is still the template)
- Concurrent in-flight requests
- Answering reverse requests (`runInTerminal`, `startDebugging`) — we do not advertise those client capabilities
- Attach, evaluate, disassemble, memory
- `tracing` (harness prints `→` / `←` JSON instead)

---

## How to run what exists

```bash
clang -g -O0 -o testdata/hello testdata/hello.c
cargo test          # framing + type unit tests (no lldb-dap)
cargo run           # smoke harness against testdata/hello (needs xcrun lldb-dap)
```

On macOS the client spawns `xcrun lldb-dap`. stderr is inherited so adapter errors show up in the same terminal.
