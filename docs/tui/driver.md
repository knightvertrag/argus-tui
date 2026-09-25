# Driver

`driver::run` is the only TUI caller of `Session`. It is one tokio task. The draw loop sends `Command` on an unbounded channel and receives `ViewModel` on a `watch` channel.

`request()` on the client must run to completion. The driver therefore never puts a session call inside `select!`. It only selects between the command channel and `session.next_event()`. Handlers run after a branch wins, so `refresh_stop_context` and `evaluate` are not cancelled. Details of why `recv` is safe to cancel and `request` is not: [`../dap-implementation.md`](../dap-implementation.md).

## Startup

```
select (biased):
  Quit or channel closed  → return (launch future is dropped; Client::drop kills lldb-dap)
  other command           → publish status "still launching"
  Session::launch         → break with the session, or publish the error and wait for Quit
```

If launch already left phase `Stopped` (a stop arrived during configuration), the driver calls the same inspect path as a later stop. Otherwise it publishes once so the footer leaves "launching".

## Steady loop

While phase is `Exited` or `Terminated`, the driver stops calling `next_event` and only reads commands. Quit disconnects. Other commands still dispatch; stepping answers `not stopped`.

Otherwise:

```
select (biased):
  Quit or channel closed  → disconnect and return
  other Command           → dispatch, awaited alone
  next_event Ok(ev)       → on_event, awaited alone
  next_event Err          → status + terminal line, wait for Quit, disconnect
```

`biased` prefers a queued command over a DAP event that is already ready.

## Stop

`SessionEvent::Stopped`, and a step method that returns with phase already `Stopped` (the stop was applied from the inbox inside `request`):

1. `refresh_stop_context` — threads and stack, focus the top frame, clear scopes / locals / registers.
2. `load_frame_variables` — locals and the `Registers` scope.
3. `evaluate` each watch, in order. A failed eval stores the error text as the value. It does not fail the stop.
4. `publish`.

`select_thread` refreshes the stack (top frame) and then reloads variables. `select_frame` does not refresh the stack; it only reloads variables and watches for the frame the user picked. Frame numbers in the prompt are the `#0` index, not the DAP frame id.

Output, thread, breakpoint, continued, and process events publish without another DAP round-trip. `publish` copies any `state.output` entries the driver has not yet copied (`output_seen`). Stderr is `TermStyle::Err`; everything else is `TermStyle::Out`. The transcript is capped at 500 lines.

## Commands that touch DAP

| Command | When not `Stopped` | Session call |
|---------|--------------------|--------------|
| Resume / StepOver / StepIn / StepOut | status `not stopped` | `resume` / `step_over` / `step_in` / `step_out`, then inspect if phase is already `Stopped`, else publish `Running` |
| ToggleBreakpoint | `not stopped` | full `set_breakpoints` for the focused file, line added or removed |
| Break { path, line, condition } | `not stopped` | same, path left as typed; a line that already exists gets its condition replaced |
| DeleteBreakpoint(id) | `not stopped` | drop that id's spec and resend the file |
| Thread(id) | `not stopped` | `select_thread` |
| Frame(index) | `not stopped` | `select_frame` of `frames[index].id` |
| Watch / Unwatch | allowed | local list; `evaluate` the new expression immediately if stopped |
| Help / Quit | — | not dispatched; the loop handles Quit, the draw loop handles Help |

`setBreakpoints` replaces the whole file. The driver reads the specs already stored for that path (`same_file`: equal path, else both canonicalize, else same file name), edits the list, and sends it back.

Toggle uses the focused frame's path. No path → status `no source`.

Echoes (`> continue`, `> break …`) are appended to the terminal transcript before the DAP call. `SessionError` becomes the status line and a `TermStyle::Error` row, then publish. The driver keeps running.

## Publish

`ViewModel::capture` needs `&mut Session` (source load) and the watch / terminal / status snapshots. Those are cloned first so the borrows do not overlap. `watch::Sender::send` replaces the draw loop's latest snapshot. The draw loop's `changed()` wakes it.

While `Running`, frames are already cleared by `enter_running`, so the next snapshot has no current line and the arrow disappears before the next stop.
