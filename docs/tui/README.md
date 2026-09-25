# TUI implementation

How the screen is built and how it drives `debugger::Session`. For the layer contract and the launch handshake, see [`../architecture.md`](../architecture.md). Session and DAP internals stay in their own docs.

| Doc | Contents |
|-----|----------|
| [`overview.md`](./overview.md) | Modules, ownership, what each pane reads |
| [`driver.md`](./driver.md) | The task that owns `Session`, commands, and publish |
| [`screen.md`](./screen.md) | Layout, theme, source view, scroll, highlighting |
| [`input.md`](./input.md) | Focus, keys, prompt grammar |

Visual target for the Code tab: [`../../screens/tui_proto.jpg`](../../screens/tui_proto.jpg).

The draw loop does not call `Session` or `Client`. It sends `command::Command` values and paints the latest `ViewModel`.
