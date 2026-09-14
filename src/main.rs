mod dap;
mod debugger;
mod logging;

use std::path::PathBuf;

use anyhow::{Context, Result, bail};

use debugger::{LaunchConfig, Session};

#[tokio::main]
async fn main() -> Result<()> {
    logging::init();

    let program = PathBuf::from("testdata/hello");
    if !program.exists() {
        bail!(
            "testdata/hello not found. Compile it first:\n  clang -g -O0 -o testdata/hello testdata/hello.c"
        );
    }
    let program = program.canonicalize()?;

    let mut config = LaunchConfig::new(&program);
    config.stop_on_entry = true;
    let mut session = Session::launch(config)
        .await
        .context("failed to launch debug session")?;

    session
        .wait_until_stopped()
        .await
        .context("failed to wait for stop")?;

    let state = session.state();
    println!("\n*** Program stopped ({:?}) ***", state.phase);
    if let Some(stopped) = &state.stopped {
        println!("Reason: {:?}", stopped.reason);
        if let Some(thread_id) = stopped.thread_id {
            println!("Thread: {thread_id}");
        }
    }

    println!("\nStack:");
    for (index, frame) in state.frames.iter().enumerate() {
        let path = frame
            .source
            .as_ref()
            .and_then(|source| source.path.as_deref())
            .unwrap_or("??");
        println!(
            "  #{index} {} at {}:{}:{}",
            frame.name, path, frame.line, frame.column
        );
    }

    if let Some(location) = session.state().location()
        && let Some(path) = &location.path
    {
        match session.sources().line(path, location.line) {
            Ok(Some(text)) => {
                println!("\n{}:{}", path.display(), location.line);
                println!("  {text}");
            }
            Ok(None) => {}
            Err(err) => println!("\n(could not read {}: {err})", path.display()),
        }
    }

    let locals = session
        .load_locals()
        .await
        .context("failed to load locals")?;
    println!("\nLocals:");
    if locals.is_empty() {
        println!("  (none)");
    } else {
        for variable in locals {
            match &variable.type_field {
                Some(ty) => println!("  {} = {} ({ty})", variable.name, variable.value),
                None => println!("  {} = {}", variable.name, variable.value),
            }
        }
    }

    let status = session.disconnect().await?;
    println!("\nlldb-dap exited: {status}");
    Ok(())
}
