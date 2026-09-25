# TUI C Debugger — Project Plan

> A professional, educational roadmap for building a modern Rust TUI frontend for debugging C programs using the Debug Adapter Protocol (DAP) and `lldb-dap`.

**Status:** Phases 0–2 are in the tree. `cargo run -- <program>` is the TUI (source, watches, breakpoints, registers, threads, call stack, terminal, `c`/`n`/`s`/`f`). `--harness` still prints one stop. Phase 3 is only partly started: source breakpoints and `evaluate` watches exist; launch configs, path mapping, and variable children do not. Implementation of the screen is [`tui/README.md`](./tui/README.md). The layer contract is [architecture.md](./architecture.md).
**Last updated:** 2026-09-25  
**Target audience:** Intermediate learner (beginner–intermediate Rust, no prior TUI or deep debugger experience)

---

## 1. Vision

Build a **sleek, informationally rich, and powerful** terminal debugger frontend with these goals:

### Initial Implementation
1. Rust-based TUI with a modern, dense, professional interface.
2. Use the **Debug Adapter Protocol (DAP)** with **`lldb-dap`** as the backend.

### Long-term Goals
1. Integrate **agentic AI** capabilities that can reason through and debug programs automatically.

### Meta Requirements
- Educational: every layer should teach valuable systems / protocol / UI concepts.
- Professional: clean architecture that can grow into a serious tool.
- Approachable: structured so a Rust beginner can make steady progress.

---

## 2. Core Architecture

Keep three clean layers:

```
┌─────────────────────────────────────┐
│           TUI (ratatui)             │  ← presentation + input
├─────────────────────────────────────┤
│     Debug Session / State Machine   │  ← pure domain logic
├─────────────────────────────────────┤
│      DAP Client (lldb-dap)          │  ← protocol + process control
└─────────────────────────────────────┘
```

**Why this separation matters**

- The TUI never speaks raw DAP.
- The same session layer can later be driven by a human UI *or* an AI agent.
- Testing, logging, and future MCP / control-plane integrations become straightforward.

---

## 3. Recommended Tech Stack

| Layer              | Choice                                      | Rationale |
|--------------------|---------------------------------------------|-----------|
| TUI                | **ratatui** + **crossterm**                 | Current standard, excellent docs, active ecosystem |
| Async runtime      | **tokio**                                   | Required for process I/O and future agent work |
| Error handling     | **anyhow** in `main`, **thiserror** in `dap` / `debugger` | `color-eyre` was dropped when the template went away |
| Logging            | **tracing** + **tracing-subscriber**        | Essential for debugging a debugger |
| Serialization      | **serde** + **serde_json**                  | DAP is JSON |
| DAP types          | Hand-written first, generate later          | Full control + high educational value |
| CLI / config       | **clap** + optional TOML / JSON5            | Launch configurations |

> **Note:** Avoid heavy “complete DAP client” crates at the start. Most are server-oriented, incomplete, or private. Building a focused client is one of the highest-value learning experiences in this project.

---

## 4. Project Bootstrap

```bash
cargo install cargo-generate
cargo generate ratatui/templates
# Choose: Event Driven Async  (or Component for more structure)
```

Then restructure into the layout below. That sketch is historical. The live tree is in [MEMORY.md](../MEMORY.md): `driver.rs` owns the session, `ui/` paints a `ViewModel`, and there is no `app.rs`.

### Suggested Directory Structure

```
src/
├── main.rs
├── app.rs                 # top-level state + event loop
├── event.rs
├── config.rs
├── dap/
│   ├── mod.rs
│   ├── framing.rs         # Content-Length protocol
│   ├── client.rs          # spawn + send/receive
│   ├── session.rs         # high-level API (launch, continue, step…)
│   └── types.rs           # request/response/event structs
├── debugger/
│   ├── mod.rs
│   ├── state.rs           # breakpoints, stopped location, threads…
│   └── source.rs          # source file cache + line mapping
└── ui/
    ├── mod.rs
    ├── layout.rs
    ├── source_view.rs
    ├── variables.rs
    ├── stack.rs
    ├── threads.rs
    ├── console.rs
    └── status_bar.rs
```

---

## 5. Phased Implementation Plan

### Phase 0 — Foundations (done)
**Goal:** Comfort with the basic tools.

- Master a simple ratatui event loop and layout.
- Learn `tokio::process::Command` and async stdin/stdout.
- Understand the DAP base protocol (header + `Content-Length` + JSON body).
- Manually drive `lldb-dap` with a tiny C program compiled `-g -O0` so you can see real message traffic.

**Exit criteria:** You can spawn a process, exchange framed JSON messages, and render a basic multi-pane UI.

---

### Phase 1 — Minimal DAP Client (done)
**Goal:** A working protocol client that can stop a program and inspect state.

Implement only what is required:

1. Spawn `lldb-dap`
2. Content-Length framing
3. `initialize` → response + `initialized` event
4. `launch` (or `attach`)
5. Handle `stopped` event
6. `stackTrace` → `scopes` → `variables`
7. `continue` / `next` / `stepIn` / `stepOut`

**Exit criteria:** From the command line (or a minimal test harness) you can launch a C program, hit a breakpoint or stop-on-entry, and print the current stack + locals.

---

### Phase 2 — First Useful TUI (done)
**Goal:** A usable interactive debugger.

What shipped, matching `screens/tui_proto.jpg` for the Code tab:

- Source pane (current line, arrow, breakpoint line numbers, C highlighting)
- Watch pane (`evaluate` in the watch context)
- Breakpoints as ID / Location / Condition rows, Watch, and Call Stack on the left
- Variables and a short Register pane on the right (`screens/tui_dense.jpg`)
- Tabs: Code, Vars, Stack, Threads, Memory, Terminal
- Keys `c` / `n` / `s` / `f` / `b` / `q`, and a `>` prompt for `break`, `watch`, `thread`, `frame`
- Footer shows `Ln` / `Col` / function, or `Running` / `Exited`

**Exit criteria:** You can step through a small C program entirely from the TUI. Met with `cargo run -- -b testdata/hello.c:14 testdata/hello` (stop reason `Breakpoint` in `main`).

---

### Phase 3 — Real Power
**Goal:** Feature parity with a solid everyday debugger.

- Source breakpoints + conditional breakpoints (first cut is in: `break file:line if cond`; no hit counts)
- Multiple threads (list and `thread <id>` exist; not a daily-driver threads UI)
- Expression evaluation in the debug console (`watch <expr>` exists; no REPL, no variable-child expansion)
- Launch configurations (`.vscode/launch.json`-style or custom)
- Source path mapping / rewriting
- Output / console filtering

**Exit criteria:** Comfortable debugging real C projects with multiple source files and reasonable complexity.

---

### Phase 4 — Professional Polish + AI Foundation
**Goal:** Production quality and agent readiness.

- Clean command / event bus so both UI and future agent drive the same session
- Persistent breakpoints and watch expressions
- Disassembly view, memory view (optional)
- Logging, configuration, and error surfaces suitable for long sessions
- Foundation for agentic control (MCP tools or custom control channel)

---

## 6. Key Learning Resources

- [ratatui book + templates](https://ratatui.rs) and official examples
- [Debug Adapter Protocol overview & specification](https://microsoft.github.io/debug-adapter-protocol/)
- `lldb-dap` itself (run with logging enabled to inspect traffic)
- Reference (study, don’t copy wholesale):
  - `debugger-cli` — excellent LLM-oriented DAP client design
  - `koan-debugger` / `debugium` — TUI + DAP examples

---

## 7. Practical Advice for a Beginner

1. **Keep v0.1 deliberately small.**  
   Source + stack + variables + step/continue is already a serious frontend.

2. **Make the DAP client emit domain events**, not raw JSON.  
   Examples: `Stopped { reason, thread_id, ... }`, `VariablesUpdated`, `Exited`.

3. **Log every DAP message** (request, response, event) at `debug` level.  
   You will consult these logs constantly.

4. Prefer **explicit state machines** over clever async control flow.

5. Write a small set of **integration tests** that launch a known C binary and assert on stop location and variable values.

6. Compile test programs with:
   ```bash
   clang -g -O0 -o hello hello.c
   # or
   gcc -g -O0 -o hello hello.c
   ```

---

## 8. Long-term AI Vision

Because the session layer is pure, the architecture naturally supports agents:

```
Agent  ──(commands)──▶  Debug Session  ◀──(events)──  DAP Client
                              ▲
                              │
                           Human TUI
```

Later you can expose the same session via:

- MCP tools
- A local JSON-RPC / control port
- An in-process agent loop

This is the pattern used by several modern “AI debugger” projects.

---

## 9. Prerequisites

- Rust toolchain (stable)
- `lldb-dap` available on `PATH`
  - Linux: usually part of the `lldb` package
  - macOS: Xcode Command Line Tools or `brew install llvm`
- Ability to compile C with debug info (`-g -O0`)

---

## 10. Suggested First Milestone Checklist

- [x] Project created from ratatui event-driven-async template
- [x] Directory structure in place
- [x] Can spawn `lldb-dap` and exchange a simple `initialize` request/response
- [x] Content-Length framing correctly implemented
- [x] Can launch a tiny C program and receive a `stopped` event
- [x] Can request `stackTrace` and print frames
- [x] Minimal TUI that shows source + current line
- [x] `n` / `s` / `c` keybindings work

---

## 11. Open Design Questions (to revisit)

- Preferred keybinding style — current choice is hybrid: single keys in the view (`c`/`n`/`s`/`f`), words on the prompt. Revisit if that splits badly.
- Support for attach vs launch first?
- How early to introduce launch configuration files?
- Whether to generate DAP types from the official JSON schema later?
- Exact shape of the future agent command surface?

---

*This document is intended as a living reference. Update the checklists and phase status as the project progresses.*
