# argus-tui

Terminal debugger for C. The UI is [ratatui](https://ratatui.rs); the backend is [`lldb-dap`](https://lldb.llvm.org/) over the [Debug Adapter Protocol](https://microsoft.github.io/debug-adapter-protocol/).

`cargo run -- <program>` is the debugger. `cargo run -- --harness` is the old smoke printer (launch `testdata/hello`, break on `main`, and print stack / locals).

## Docs

- [MEMORY.md](MEMORY.md) — LLM pickup (layers, decisions, APIs, gotchas)
- [Architecture](docs/architecture.md) — layers, integration, launch/debug flow
- [Session implementation](docs/session-implementation.md) — `debugger::Session` internals
- [DAP implementation](docs/dap-implementation.md) — framing, types, client internals
- [TUI implementation](docs/tui/README.md) — driver, screen, input
- [Project plan](docs/tui-c-debugger-plan.md) — roadmap (phases 0–2 done)

## Run

```bash
clang -g -O0 -o testdata/hello testdata/hello.c
cargo test
cargo clippy --all-targets -- -D warnings

cargo run -- -b testdata/hello.c:14 testdata/hello
cargo run -- --stop-on-entry testdata/hello
cargo run -- --harness
```

TUI logs append to `argus-tui.log` in the working directory (`RUST_LOG`, default `argus_tui=info`; `debug` for DAP command/event names, `trace` for JSON bodies). The harness logs to stderr. `lldb-dap` stderr is traced at `debug` under target `lldb_dap`, not drawn on the screen.

In the TUI, `c` / `n` / `s` / `f` continue and step, `b` toggles a breakpoint, `1`–`5` switch views, `?` lists keys. Enter or `:` focuses the prompt (`break file:line if cond`, `watch`, `delete`, `thread`, `frame`). `q`, Esc, or Ctrl-C quits.

Pass breakpoint paths as they appear in the debug info. Rewriting them with `canonicalize` can change casing (`Code` vs `COde`) and lldb will not verify the breakpoint.

This repo started from the [Ratatui event-driven async template](https://github.com/ratatui/templates/tree/main/event-driven-async).

## License

Copyright (c) knightvertrag <amiapu1@gmail.com>

This project is licensed under the MIT license ([LICENSE] or <http://opensource.org/licenses/MIT>)

[LICENSE]: ./LICENSE
