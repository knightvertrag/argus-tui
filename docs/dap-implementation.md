# DAP implementation notes

Low-level description of the DAP stack (framing, JSON types, `Client`). For layering and flow, see [`architecture.md`](./architecture.md). For `debugger::Session`, see [`session-implementation.md`](./session-implementation.md). Spec: [Debug Adapter Protocol](https://microsoft.github.io/debug-adapter-protocol/specification).

---

## Layout

```
src/dap/
  mod.rs           Client / Incoming / ClientError re-exports; pub mod types
  framing.rs       Content-Length encode/decode (no serde)
  client.rs        process + seq + inbox
  types/
    mod.rs         Message envelope, Request trait, ProtocolError
    requests.rs    command markers + args/response bodies
    events.rs      Event enum + parse_event
    objects.rs     Source, StackFrame, Variable, Capabilities, …
    tests.rs       JSON roundtrips (no adapter)
```

`debugger::Session` is the intended consumer of `dap::Client`. The crate is a binary (`mod dap;` / `mod debugger;` from `main.rs`); there is no `lib.rs`.

---

## Framing (`framing.rs`)

On-wire shape (headers ASCII, body UTF-8), same idea as LSP:

```
Content-Length: 119\r\n
\r\n
{"seq":1,"type":"request","command":"next","arguments":{"threadId":3}}
```

`Content-Length` is the **byte** length of the JSON body, not character count.

| API | Role |
|-----|------|
| `encode(body) -> Vec<u8>` | `Content-Length: {n}\r\n\r\n` + body |
| `write(w, body)` | encode, `write_all`, `flush` |
| `read(r) -> Vec<u8>` | header loop then `read_exact(len)` |

Read behaviour:

- `read_until(b'\n')`, strip trailing `\r`.
- Blank line ends headers.
- Header name matched case-insensitively via `split_once(':')` + `eq_ignore_ascii_case("Content-Length")`. Other headers (`Content-Type`) are ignored.
- Leading/trailing spaces around the length are trimmed.
- Caps: **1 MiB** total header bytes, **32 MiB** body. Oversize → `HeadersTooLarge` / `BodyTooLarge`.
- `read_until` returning 0, or `read_exact` hitting EOF → `UnexpectedEof`.
- Non-UTF-8 header lines → `InvalidHeader`. Unparseable length → `InvalidLength`. Missing length after the blank line → `MissingContentLength`.

Tokio `AsyncBufRead` / `AsyncWrite` only. Unit tests use `BufReader<Cursor<Vec<u8>>>` and `Vec<u8>` as `AsyncWrite`. Framing does not parse JSON.

---

## Types: envelope (`types/mod.rs`)

```rust
#[serde(tag = "type")]
enum Message {
    #[serde(rename = "request")]  Request(RequestMessage),
    #[serde(rename = "response")] Response(ResponseMessage),
    #[serde(rename = "event")]    Event(EventMessage),
}

type Seq = i64;
```

Internally tagged newtype variants flatten the inner struct, so the JSON is `{"type":"request","seq":1,"command":"initialize",…}` — not nested.

`command` / `event` stay `String`. `arguments` / `body` stay `Option<Value>`. That is required so:

- reverse requests and unknown events still deserialize
- a failed response (`success: false`) is not forced into `R::Response`

`ResponseMessage.request_seq` is **not** camelCased. The spec field is `request_seq`. Envelope structs therefore do **not** use `rename_all = "camelCase"`. Payload structs do.

Optional envelope fields use `#[serde(default, skip_serializing_if = "Option::is_none")]`.

### `Request` trait

```rust
pub trait Request {
    const COMMAND: &'static str;
    type Arguments: Serialize;
    type Response: DeserializeOwned + Default + 'static;
}
```

Commands are uninhabited marker enums (`pub enum Initialize {}`) plus `impl Request`. The client is `request::<Initialize>(args) -> Capabilities`. This is a *client* pattern; a giant `Command` enum is what DAP *servers* want.

`RequestMessage::new::<R>(seq, args)` serializes arguments and **omits** `null` and `{}` so `configurationDone` / empty `disconnect` have no `arguments` field.

### Success vs failure

`ResponseMessage::decode_success::<R>()`:

- `success == false` → `ProtocolError::Failed` (optional `ErrorBody` boxed to keep the error type small).
- missing / `null` body → `R::Response::default()` (so omitted `initialize` body is empty `Capabilities`, omitted `launch` body is `()`).
- empty object `{}` is treated as `()` only when `R::Response` is `()` (`TypeId` check). Other types still serde-decode the object.
- otherwise `from_value`.

`ProtocolError::Unexpected` exists for callers that pattern-match the wrong `Message` variant; the client currently uses `ClientError::UnexpectedResponse` instead.

---

## Types: requests (`types/requests.rs`)

| Marker | Command | Arguments | Success body |
|--------|---------|-----------|--------------|
| `Initialize` | `initialize` | `InitializeArguments` | `Capabilities` |
| `Launch` | `launch` | `LaunchArguments` | `()` |
| `ConfigurationDone` | `configurationDone` | `()` | `()` |
| `SetBreakpoints` | `setBreakpoints` | `SetBreakpointsArguments` | `SetBreakpointsResponse` |
| `Threads` | `threads` | `()` | `ThreadsResponse` |
| `StackTrace` | `stackTrace` | `StackTraceArguments` | `StackTraceResponse` |
| `Scopes` | `scopes` | `ScopesArguments` | `ScopesResponse` |
| `Variables` | `variables` | `VariablesArguments` | `VariablesResponse` |
| `Continue` | `continue` | `ContinueArguments` | `ContinueResponse` |
| `Next` | `next` | `NextArguments` | `()` |
| `StepIn` | `stepIn` | `StepInArguments` | `()` |
| `StepOut` | `stepOut` | `StepOutArguments` | `()` |
| `Evaluate` | `evaluate` | `EvaluateArguments` (`expression`, `frameId`, `context: "watch"`) | `EvaluateResponse` (`result`, optional `type`, `variablesReference`) |
| `Disconnect` | `disconnect` | `DisconnectArguments` | `()` |

The session sends these. The harness path uses initialize, launch, configurationDone, threads, stackTrace, scopes, variables, disconnect. The TUI driver also sends setBreakpoints, continue/next/stepIn/stepOut, and evaluate.

### Serde traps

- `adapterID` / `clientID` are `ID`, not `Id`. Explicit `rename` on those fields. `supportsANSIStyling` is also explicit.
- `lines_start_at1` → `linesStartAt1` via `camelCase` (spec name).
- `Variable.type_field` serializes as `"type"`.
- DAP ids (`threadId`, `frameId`, `variablesReference`, `seq`) are `i64`.
- Payload optionals: `Option<T>` + skip if `None`. Capability **bools** use `#[serde(default)]` (omitted means false).

### `InitializeArguments::new`

Advertises: `clientID`/`clientName` = `argus-tui`, `pathFormat` = `path`, 1-based lines/columns, `supportsVariableType`. Does **not** set `supportsRunInTerminalRequest` or `supportsStartDebuggingRequest`.

### `LaunchArguments`

DAP only specifies `noDebug` / `__restart`. We model `lldb-dap` fields we use (`program`, `args`, `cwd`, `env` as `BTreeMap<String, String>`, `stopOnEntry`) plus `#[serde(flatten)] extra` for everything else (`initCommands`, …). `env` is object form only.

### `Capabilities`

Named flags the session will branch on, all `bool` + default:

`supports_configuration_done_request`, `supports_conditional_breakpoints`, `supports_set_variable`, `supports_terminate_request`, `support_terminate_debuggee` (spec spelling, no “s” on support), `supports_single_thread_execution_requests`, `supports_delayed_stack_trace_loading`, `supports_cancel_request`.

Unknown keys go to `extra: Map<String, Value>` via flatten, so an `lldb-dap` initialize response is not lossy.

---

## Types: events (`types/events.rs`)

```rust
enum Event {
    Initialized,
    Stopped(StoppedEvent),
    Continued(ContinuedEvent),
    Exited(ExitedEvent),
    Terminated(TerminatedEvent),
    Thread(ThreadEvent),
    Output(OutputEvent),
    Process(ProcessEvent),
    Breakpoint(BreakpointEvent),
    Unknown { event: String, body: Option<Value> },
}
```

`EventMessage::parse()` matches on the `event` string. Unknown names (`module`, `capabilities`, …) become `Unknown` — not an error. `initialized` ignores body. `terminated` body is optional (`Default`). `stopped` / `output` / `process` / … require a body and fail with `ProtocolError::Decode` if it is missing or malformed.

Stringly spec enums (`StoppedReason`, `OutputCategory`, …) use `#[serde(untagged)] Other(String)` so adapter extensions deserialize. `SteppingGranularity` uses `#[serde(other)] Unknown` (does not keep the original string).

---

## Types: objects (`types/objects.rs`)

Shared structs used by requests and events: `Source`, `StackFrame`, `Scope`, `Variable`, `Thread`, `Breakpoint`, `SourceBreakpoint`, `ErrorMessage`, `ErrorBody`, `Capabilities`, `SteppingGranularity`.

The spec type `Message` (structured error) is named `ErrorMessage` so it does not collide with the wire `Message` enum.

---

## Client (`client.rs`)

```rust
pub struct Client {
    child: Option<Child>,          // None after shutdown()
    stdin: ChildStdin,
    frames: UnboundedReceiver<Result<Message, ClientError>>,
    next_seq: Seq,                 // starts at 1
    inbox: VecDeque<Incoming>,
}

pub enum Incoming {
    Event(Event),
    ReverseRequest(RequestMessage),
}
```

**Spawn:** `xcrun lldb-dap`, stdin/stdout/stderr piped. Failure → `ClientError::Spawn`. Missing pipes → `MissingPipe`. Must run inside a tokio runtime.

Stdout is owned by a task. That task is the only caller of `framing::read`. It sends complete `Message`s on an unbounded channel. `framing::read` is not cancellation-safe (a dropped read loses bytes already pulled into a local buffer). `recv().await` only waits on the channel, so the TUI may cancel it. Do not cancel `request()`: the matching response would sit on the channel and the next read would treat it as unexpected.

Stderr lines are traced at debug with target `lldb_dap` so they do not land on the alternate screen.

**`request::<R>`:**

1. Take `next_seq`, increment.
2. `RequestMessage::new::<R>`, wrap `Message::Request`, `serde_json::to_vec`, `framing::write`.
3. Loop `next_frame` (channel recv):
   - `Response` with matching `request_seq` → `decode_success::<R>()`
   - other `Response` → `UnexpectedResponse` (no concurrent requests)
   - `Event` → parse, push `Incoming::Event`
   - `Request` → push `Incoming::ReverseRequest` (not answered)
   - channel closed → `ClientError::Closed`

**`drain_inbox`:** take queued `Incoming` values without blocking. Session uses this after every DAP `request()`; see [`session-implementation.md`](./session-implementation.md).

**`recv`:** pop inbox if non-empty; else take one frame from the channel. A `Response` here is an error (`expected: 0` — no pending request).

**`shutdown(self)`:** `take()` the `Child`, `wait()`. After this, `Drop` does not kill.

**`Drop`:** `start_kill()` if the child is still owned, so a failed session does not leak `lldb-dap`. The reader task ends when stdout hits EOF.

The client logs via `tracing` (not stdout). Where those lines go depends on the process: stderr for `--harness`, `argus-tui.log` for the TUI. DEBUG: `seq` / `command` / `success` / event name. TRACE: JSON frame body. WARN: reverse requests (still queued, not answered). INFO: spawn and adapter exit status.

`ClientError` wraps spawn, framing, I/O, JSON, `ProtocolError`, and unexpected responses.

---

## Tests

`cargo test` compiles the binary with `cfg(test)` (no library target).

- **Framing:** roundtrip, extra headers, spaced `Content-Length`, two frames, missing length, partial header/body EOF, invalid length.
- **Client reader:** two back-to-back event frames delivered on the channel; cancelling `recv` leaves the first frame queued.
- **Types:** envelope JSON (including `request_seq` underscore), initialize advertise/decode + `extra` flags, failed response → `ProtocolError::Failed`, `()` body for missing/`{}`, event parse (`initialized`, `stopped`/`entry`, `output`, unknown `module`), camelCase ids, launch flatten extra.

No unit test spawns `lldb-dap`. Session tests live under `src/debugger/`. End-to-end is `cargo run -- --harness` or `cargo run -- -b file:line program`.

---

## Error map

```
FramingError  ──► ClientError::Framing
serde_json    ──► ClientError::Json
ProtocolError ──► ClientError::Protocol   (Failed / Decode)
io::Error     ──► ClientError::Io  or Spawn
ClientError   ──► SessionError::Client
anyhow        ◄── main.rs Context
```

Keep `thiserror` in `dap::*`. Do not use `color_eyre` inside the protocol modules.

---

## Known sharp edges

- One in-flight request. A second `request()` while one is pending is not supported (API is `&mut self` and sequential).
- Reverse requests are queued and ignored. If an adapter ever sent `runInTerminal` and blocked on the reply, the session would stall. We do not advertise the capability.
- `recv`’s unexpected-response error uses `expected: 0` rather than a dedicated variant.
- Cancelling `request()` is still unsafe. Only the channel wait inside `recv` is cancellation-safe.
- `Event`’s `Serialize` form is the Rust enum (`{"Stopped":{…}}`, `{"Unknown":{…}}`), not the DAP wire shape. The wire shape is `EventMessage`.
- First `stopped` after `stopOnEntry` may have reason `exception` on Apple `lldb-dap`.
- Spawn is macOS-centric (`xcrun lldb-dap`). Linux would want `lldb-dap` on `PATH`.
- `src/dap/types/mod.rs` still `allow(dead_code, unused_imports)` because some request markers are only used via the session.
