#![allow(dead_code)]

mod client;
pub mod types;

struct Dap {
    dap: tokio::process::Child,
}
