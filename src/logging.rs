use std::io::IsTerminal;

use tracing_subscriber::EnvFilter;
use tracing_subscriber::fmt;

/// Install a stderr subscriber. No-op extra if tests never call this.
///
/// Default filter when `RUST_LOG` is unset: `argus_tui=info`.
pub fn init() {
    let filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("argus_tui=info"));
    fmt()
        .with_env_filter(filter)
        .with_writer(std::io::stderr)
        .with_target(true)
        .with_ansi(std::io::stderr().is_terminal())
        .init();
}
