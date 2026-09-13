mod dap;

use std::path::PathBuf;

use anyhow::{Context, Result, bail};
use serde::Serialize;

use dap::types::{
    ConfigurationDone, Disconnect, DisconnectArguments, Event, Initialize, InitializeArguments,
    Launch, LaunchArguments, Request, StoppedReason,
};
use dap::{Client, Incoming};

#[tokio::main]
async fn main() -> Result<()> {
    let program = PathBuf::from("testdata/hello");
    if !program.exists() {
        bail!(
            "testdata/hello not found. Compile it first:\n  clang -g -O0 -o testdata/hello testdata/hello.c"
        );
    }
    let program = program.canonicalize()?;

    let mut client = Client::spawn()?;

    let _caps = send::<Initialize>(&mut client, InitializeArguments::new("lldb-dap")).await?;

    let mut launch = LaunchArguments::new(program.to_string_lossy());
    launch.stop_on_entry = Some(true);
    send::<Launch>(&mut client, launch).await?;

    recv_until(&mut client, |incoming| match incoming {
        Incoming::Event(Event::Initialized) => {
            println!("✓ initialized event");
            true
        }
        Incoming::Event(event) => {
            println!(
                "← event {}",
                serde_json::to_string(event).unwrap_or_default()
            );
            false
        }
        Incoming::ReverseRequest(req) => {
            println!("← reverse-request {}", req.command);
            false
        }
    })
    .await?;

    send::<ConfigurationDone>(&mut client, ()).await?;

    recv_until(&mut client, |incoming| match incoming {
        Incoming::Event(Event::Stopped(stopped)) => {
            println!("\n*** Program stopped ***");
            println!("Reason: {}", reason_label(&stopped.reason));
            if let Some(thread_id) = stopped.thread_id {
                println!("Thread: {thread_id}");
            }
            true
        }
        Incoming::Event(event) => {
            println!(
                "← event {}",
                serde_json::to_string(event).unwrap_or_default()
            );
            false
        }
        Incoming::ReverseRequest(req) => {
            println!("← reverse-request {}", req.command);
            false
        }
    })
    .await?;

    let _ = send::<Disconnect>(
        &mut client,
        DisconnectArguments {
            terminate_debuggee: Some(true),
            ..Default::default()
        },
    )
    .await;

    let status = client.shutdown().await?;
    println!("lldb-dap exited: {status}");
    Ok(())
}

async fn send<R: Request>(client: &mut Client, args: R::Arguments) -> Result<R::Response>
where
    R::Arguments: Serialize,
    R::Response: Serialize,
{
    println!("→ {} {}", R::COMMAND, serde_json::to_string(&args)?);
    let response = client
        .request::<R>(args)
        .await
        .with_context(|| format!("{} failed", R::COMMAND))?;
    println!("← {} {}", R::COMMAND, serde_json::to_string(&response)?);
    Ok(response)
}

async fn recv_until(client: &mut Client, mut done: impl FnMut(&Incoming) -> bool) -> Result<()> {
    loop {
        let incoming = client.recv().await.context("failed to read DAP message")?;
        if done(&incoming) {
            return Ok(());
        }
    }
}

fn reason_label(reason: &StoppedReason) -> String {
    match reason {
        StoppedReason::Step => "step".into(),
        StoppedReason::Breakpoint => "breakpoint".into(),
        StoppedReason::Exception => "exception".into(),
        StoppedReason::Pause => "pause".into(),
        StoppedReason::Entry => "entry".into(),
        StoppedReason::Goto => "goto".into(),
        StoppedReason::FunctionBreakpoint => "function breakpoint".into(),
        StoppedReason::DataBreakpoint => "data breakpoint".into(),
        StoppedReason::InstructionBreakpoint => "instruction breakpoint".into(),
        StoppedReason::Other(other) => other.clone(),
    }
}
