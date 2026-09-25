# Overview

The TUI is four pieces. Only the driver talks to the session.

```
src/main.rs          args, terminal lifecycle, draw loop
src/event.rs         crossterm keys and resizes (no fixed tick)
src/driver.rs        owns Session; Command in, ViewModel out
src/command.rs       prompt line → Command
src/view.rs          ViewModel::capture from SessionState + watches
src/highlight.rs     one C source line → tokens
src/ui/              layout, theme, panes, keys, help
```

```
key / prompt ──Command──▶ driver ──owns──▶ Session ──▶ Client ──▶ lldb-dap
                              │
                              └── watch::Sender<ViewModel>
                                        │
draw loop ◀── watch::Receiver ──────────┘
     │
     └── ui::draw(ViewModel, UiState)
```

`UiState` (tab, focus, prompt buffer, scroll) stays in the draw loop. It is not part of the session and is not sent to the driver. Scroll and help never become DAP calls.

## What each region reads

| Region | `ViewModel` fields | Session source |
|--------|--------------------|----------------|
| Code | `source_lines`, `line`, `column`, `function`, `breakpoint_lines`, `phase` | `SourceCache` for the focused frame path; `location()` |
| File bar | `file_name`, phase footer | focused frame path |
| Code gutter | `breakpoint_lines` | breakpoint specs whose path is the same file as the frame |
| Watch | `watches` | driver-owned expressions; values from `Session::evaluate` on each stop |
| Breakpoints | `breakpoints` | `BoundBreakpoint`: adapter id / verified / line, condition from the spec. Drawn as ID / Location / Condition rows. |
| Variables | `locals` | non-register locals from `load_frame_variables` |
| Register | `registers`, `registers_unavailable` | variables loaded from the scope named `Registers` |
| Call stack | `frames` | `state.frames`, focused frame in magenta. Also a full-height Stack tab. |
| Threads tab | `threads` | `state.threads`, mark `focused_thread` |
| Memory tab | — | no session data yet |
| Terminal tab | `terminal` | new `state.output` events, plus command echoes and session errors |
| Prompt | `status`, local `UiState.message` | last driver status string |

`ViewModel::capture` copies those fields out of `SessionState`, then calls `sources().load` for the focused path. Widgets never see DAP types.

## Empty states

- Stopped with no source path (a loader frame such as `_dyld_start`): footer `Stopped in {function} (no source)`. The body repeats that line. `--stop-on-entry` does not stop there; it breaks on `main`.
- Source file unreadable: `source_error` in the body; the footer still shows `Ln` / `Col` when the frame has them.
- Watch list empty: the pane is blank.
- No breakpoint rows: the table header only.
- Stopped, scopes loaded, none named `Registers`: `(unavailable)`. Before the first inspect, the pane stays blank (`registers_unavailable` is false).

## Process lifetime

`run_tui` checks the program exists, canonicalizes **only the program path**, opens `argus-tui.log`, then `ratatui::try_init`. `ratatui::restore` runs when the draw loop returns. The driver is a spawned task. When it finishes, the draw loop ends. Quit sends `Command::Quit`; the driver calls `Session::disconnect` (terminate the debuggee, then wait for `lldb-dap`).

Breakpoint paths from `-b` are stored as typed. See [`../session-implementation.md`](../session-implementation.md) for why they are not canonicalized.
