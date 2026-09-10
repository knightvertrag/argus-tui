use tokio::process::Command;

use crate::dap::Dap;

pub fn spawn_dap() -> Dap {
    let dap = Command::new("xcrun lldb-dap")
        .stdout(std::process::Stdio::piped())
        .stdin(std::process::Stdio::piped())
        .spawn()
        .expect("Failed to spawn lldb-dap process");
    Dap { dap }
}
