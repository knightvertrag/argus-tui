# Screen

`ui::draw` fills the frame with the background color, splits chrome, draws the active body, then the tab strip, the prompt, and the help overlay if it is open.

## Chrome

`ui/layout.rs`, matching `screens/tui_dense.jpg`:

| Region | Constraint |
|--------|------------|
| File bar | height 3. Title is the source file name. The inner line is the phase footer (`Ln` / `Col` / function, or `Running` / `Exited`). |
| Body | `Min(1)` |
| Tabs | height 3 (bordered active tab) |
| Command | height 3, bordered pane titled Command. The inner line is `>` plus the prompt or status. The border is magenta while the prompt is focused. |

Code tab (`ui/layout.rs::code_view`):

| Column | Width | Panes |
|--------|-------|--------|
| Left | 24% | Breakpoints (fixed height, ID / Location / Condition rows), Watch, Call Stack |
| Center | 50% | Code |
| Right | 26% | Variables (locals), Register (fixed height) |

Other tabs replace the body with one full-width pane: Vars, Stack, Threads, Memory, Terminal. The file bar, tabs, and prompt stay. Memory has no session data yet; the pane says `(not available)`.

## Theme

`ui/theme.rs`, `Color::Rgb`, chosen to match the prototype:

| Token | Use |
|-------|-----|
| Background `13,17,23` | panes |
| Border `48,54,61` | pane borders |
| Title `88,196,214` | pane titles (cyan) |
| Magenta `198,120,221` | keywords, preprocessor, breakpoint ids, active tab border, breakpoint line numbers |
| Green `152,195,121` | strings, watch values, register values, conditions |
| Yellow `229,192,123` | numbers in source |
| Current line `42,46,56` | full-width bar on the stopped line |
| Cyan `86,182,194` | execution arrow, prompt cursor, focused thread/frame marker |
| Dim `106,114,128` | line numbers, comments, help hint |
| Red `224,108,117` | stderr and session errors in the terminal pane |

Panes are `Block::bordered()` with a cyan title. The active tab is its own box with a magenta border (`[1] Code`, then Vars, Stack, Threads, Memory, Terminal). Inactive tabs are dim text on the same row. The stock ratatui `Tabs` underline is not used. The stopped-line arrow is magenta. Focused call-stack rows are magenta. Register and variable values are yellow.

## Code pane

`ui/code.rs`. The last inner row is the footer (`view::code_footer`):

| Phase | Footer |
|-------|--------|
| `Stopped` with source | `Ln {line}, Col {column}    fn {name}` |
| `Stopped` without source | `Stopped in {function} (no source)` |
| `Running` | `Running` |
| `Exited` | `Exited ({code})` |
| `Terminated` | `Terminated` |
| `Starting` | status, or `Starting` |

Each source row is a gutter plus highlighted text. The gutter is a 4-wide line number, a space, then a magenta `→` on the current line only (phase `Stopped` and `has_source`). Breakpoint line numbers are magenta. The current line's background is the gray bar across the whole inner width. The `Ln` / `Col` line lives on the file bar, not inside this pane.

The pane follows the program counter unless `UiState.code_stick` is set. `j` / `k` set the stick flag. The draw loop clears it when `line` or `phase` changes, then the next draw recenters on the current line.

Highlighting (`highlight.rs`) is a scan per line, not syntect:

- `//` to end of line, and `/* */` with `in_block` carried from previous lines. The code pane scans from line 0 up to the first visible line so a block comment that started off-screen stays a comment.
- A line whose first token is `#` is preprocessor (magenta) until a comment.
- Strings and character literals, with backslash escapes.
- Numbers, including `0x` hex and a `u`/`U`/`l`/`L` suffix.
- C keywords (magenta). Other identifiers stay plain.

## Side panes and list tabs

Watch rows are `{expression} {value}` on one line. The value is green. `unwatch` is still 1-based in list order; the index is not drawn.

Breakpoints are label/value rows, not a three-column table: `ID`, `Location` (`filename:line`), `Condition`. Unverified locations are dim. Extra breakpoints stack under the first and clip when the pane is full.

Variables are locals as `{type} {name} = {value}` (the type is omitted when the adapter did not send one). The value is yellow. The Vars tab scrolls the same list.

The Register pane is the first registers that fit, name left and value right in yellow. `j` / `k` on the Code tab scroll the source, not this list.

Call-stack labels are `#{index} {name} at {file}:{line}`. The focused frame is magenta. The Code tab shows the top of the stack; the Stack tab scrolls the full list.

Threads use a cyan `→` on the focused row.

The terminal pane sticks to the bottom unless the user scrolls (`stick_to_end`). New output after a stop clears the stick flag along with the source stick, so the pane jumps back to the tail.

## Help

`?` draws a centered overlay (`Clear`, magenta border, title `Help`) listing the keys in [`input.md`](./input.md). Esc or `?` closes it. `q` while it is open quits.

## Tests

`ui::tests::code_view_matches_the_prototype_panes` renders a fixture `ViewModel` on `TestBackend` (120×40, no `lldb-dap`). It checks the pane titles, `main.c`, `main.c:8`, a register value, `fn main`, a magenta `→`, a yellow register value, and a magenta cell on the tab row. Cell colors are matched per cell: concatenating `symbol()` and using the byte index is wrong for `→` and `│`.
