use std::fs::OpenOptions;
use std::io::IsTerminal;

use tracing_subscriber::EnvFilter;
use tracing_subscriber::fmt;

/// Stderr subscriber for the smoke harness. The TUI uses [`init_file`] instead,
/// because stderr writes tear the alternate screen.
pub fn init_stderr() {
    let filter = filter();
    fmt()
        .with_env_filter(filter)
        .with_writer(std::io::stderr)
        .with_target(true)
        .with_ansi(std::io::stderr().is_terminal())
        .init();
}

/// Append-only log in the working directory (`argus-tui.log`).
pub fn init_file() -> std::io::Result<()> {
    let file = OpenOptions::new()
        .create(true)
        .append(true)
        .open("argus-tui.log")?;
    fmt()
        .with_env_filter(filter())
        .with_writer(std::sync::Mutex::new(file))
        .with_target(true)
        .with_ansi(false)
        .init();
    Ok(())
}

fn filter() -> EnvFilter {
    EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("argus_tui=info"))
}
