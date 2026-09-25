# Input

Two focuses. **View** is the default, so `c` / `n` / `s` / `f` do not need the prompt. **Prompt** is the `>` line in the prototype, after Enter or `:`.

`event.rs` forwards crossterm events. The draw loop handles `Key` only. Key releases are ignored. Repeats are accepted, so held Backspace keeps deleting. Resize is a redraw with no other work.

The draw loop's `select` is `biased`: if the driver task has finished, that wins; otherwise a key; otherwise a new `ViewModel`. A new `line` or `phase` clears `code_stick` and `list_stick`.

## View keys

| Key | Effect |
|-----|--------|
| `c` | `Command::Resume` |
| `n` | `StepOver` |
| `s` | `StepIn` |
| `f` | `StepOut` |
| `b` | `ToggleBreakpoint` on the current source line |
| `1` … `6` | Code, Vars, Stack, Threads, Memory, Terminal. Switching tabs resets list scroll. |
| `j` / Down, `k` / Up | Scroll the source on the Code tab, otherwise the active list. Sets the stick flag. |
| Enter or `:` | Focus the prompt, clear the local message |
| `?` | Help overlay |
| `q`, Esc, Ctrl-C | `Command::Quit` once (`UiState.quitting` drops further keys) |

Ctrl-C quits from the prompt and from the help overlay as well.

## Prompt

A line editor. The cursor is a byte index walked on char boundaries. Left / Right move by one char. Backspace deletes the previous char. Ctrl-U clears the line. Esc returns to view focus and clears the buffer. Enter parses and, on success, clears the line and stays in the prompt.

`command::parse` trims. An empty line is `Ok(None)`.

| Words | Command |
|-------|---------|
| `c`, `cont`, `continue` | Resume |
| `n`, `next` | StepOver |
| `s`, `step` | StepIn |
| `f`, `finish` | StepOut |
| `q`, `quit` | Quit |
| `help`, `?` | Help (opens the overlay; not sent to the driver) |
| `break`, `b` | `file:line` or `file:line if condition`. The split is the substring ` if `. Line numbers start at 1. `rsplit_once(':')` so a single colon separates the line. |
| `delete`, `d` | breakpoint id (the ID column, not the row number) |
| `watch`, `w` | the rest of the line is the expression |
| `unwatch` | 1-based index |
| `thread`, `t` | thread id shown in the threads pane |
| `frame` | 0-based stack index (`#0`) |

Unknown words and bad arguments set `UiState.message` and leave the line in place. They do not call the session. The prompt shows that message in place of the help hint. A later `ViewModel` status is what the driver publishes; the local message stays until the next successful command or until the user focuses the prompt again.

`break` / `delete` / `watch` / `thread` / `frame` are prompt-only. The single key `b` is toggle, not `break file:line`.

## Args

`parse_args` in `main.rs`, no clap:

```
argus-tui [--harness] [--stop-on-entry] [-b file:line]... <program> [args...]
```

`--harness` wins and ignores the rest. It always launches `testdata/hello` with `stop_on_entry: true`.

Without `--harness`, a program path is required. Repeated `-b` for the same path group into one `(path, lines)` entry on `LaunchConfig`. `--` starts positional args. `--stop-on-entry` defaults off.
