# argus-tui — agent rules

Project memory for LLM pickup is **[MEMORY.md](./MEMORY.md)**. Read it before non-trivial work. Prefer it over the long roadmap if they disagree.

## Hard rules

- Three layers: TUI → `debugger::Session` → `dap::Client` → `lldb-dap`. Do not skip.
- TUI/harness/agent never build DAP JSON or parse `Content-Length`.
- Session is the only `Client` consumer. Domain events wrap DAP types; they do not replace them.
- Hand-written DAP subset. Do not add `dap`/`dap-types` crates.
- One in-flight DAP request. Do not advertise reverse-request capabilities.
- `thiserror` in `dap`/`debugger`; `anyhow` in `main`. Tracing for logs, not `println` in the client.
- `src/main.rs` is the TUI. `cargo run -- --harness` is the Session smoke printer. Do not put DAP JSON in the UI or the driver.
- Implementation detail: `docs/session-implementation.md` and `docs/dap-implementation.md`. Integration/flow: `docs/architecture.md`.
