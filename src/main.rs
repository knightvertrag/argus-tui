mod dap;

use anyhow::{Context, Result, bail};
use serde_json::{Value, json};
use std::io::{BufRead, BufReader, Read, Write};
use std::path::PathBuf;
use std::process::{Command, Stdio};

fn main() -> Result<()> {
    // ---------------------------------------------------------------
    // 1. Locate the program we want to debug
    // ---------------------------------------------------------------
    let program = PathBuf::from("testdata/hello");
    if !program.exists() {
        bail!(
            "testdata/hello not found. Compile it first:\n  clang -g -O0 -o testdata/hello testdata/hello.c"
        );
    }
    let program = program.canonicalize()?;

    // ---------------------------------------------------------------
    // 2. Spawn lldb-dap
    // ---------------------------------------------------------------
    let mut child = Command::new("xcrun")
        .arg("lldb-dap")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit()) // so we see lldb-dap errors
        .spawn()
        .context("failed to spawn lldb-dap. Is it on your PATH?")?;

    let mut stdin = child.stdin.take().unwrap();
    let stdout = child.stdout.take().unwrap();
    let mut reader = BufReader::new(stdout);

    let mut seq = 1u64;

    // ---------------------------------------------------------------
    // Helper: send a DAP request
    // ---------------------------------------------------------------
    let mut send = |command: &str, arguments: Value| -> Result<()> {
        let body = json!({
            "seq": seq,
            "type": "request",
            "command": command,
            "arguments": arguments,
        });
        seq += 1;

        let body_str = serde_json::to_string(&body)?;
        let msg = format!("Content-Length: {}\r\n\r\n{}", body_str.len(), body_str);

        print!("→ {}", body_str);
        println!();
        stdin.write_all(msg.as_bytes())?;
        stdin.flush()?;
        Ok(())
    };

    // ---------------------------------------------------------------
    // Helper: read one complete DAP message
    // ---------------------------------------------------------------
    let mut read_message = || -> Result<Value> {
        // Read headers
        let mut content_length = None;
        loop {
            let mut line = String::new();
            let n = reader.read_line(&mut line)?;
            if n == 0 {
                bail!("lldb-dap closed stdout unexpectedly");
            }
            let line = line.trim_end_matches(['\r', '\n']);
            if line.is_empty() {
                break;
            }
            if let Some(rest) = line.strip_prefix("Content-Length:") {
                content_length = Some(rest.trim().parse::<usize>()?);
            }
        }

        let len = content_length.context("missing Content-Length header")?;
        let mut buf = vec![0u8; len];
        reader.read_exact(&mut buf)?;

        let value: Value = serde_json::from_slice(&buf)?;
        println!("← {}", serde_json::to_string_pretty(&value)?);
        Ok(value)
    };

    // ---------------------------------------------------------------
    // 3. initialize
    // ---------------------------------------------------------------
    send(
        "initialize",
        json!({
            "clientID": "cdebugger",
            "clientName": "C Debugger TUI",
            "adapterID": "lldb-dap",
            "pathFormat": "path",
            "linesStartAt1": true,
            "columnsStartAt1": true,
            // You can claim more capabilities later
            "supportsVariableType": true,
            "supportsVariablePaging": false,
        }),
    )?;

    // 2. Wait ONLY for the initialize response
    loop {
        let msg = read_message()?;
        if msg["type"] == "response" && msg["command"] == "initialize" {
            assert!(
                msg["success"].as_bool().unwrap_or(false),
                "initialize failed: {msg}"
            );
            break;
        }
    }

    // 3. Send launch immediately. Do NOT wait for initialized first.
    send(
        "launch",
        json!({
            "program": program,
            "stopOnEntry": true,
        }),
    )?;
    // 4. Drain until we have both initialized + launch response
    let mut got_initialized = false;
    let mut got_launch = false;
    while !(got_initialized && got_launch) {
        let msg = read_message()?;
        match msg["type"].as_str() {
            Some("event") if msg["event"] == "initialized" => {
                got_initialized = true;
                println!("✓ initialized event");
            }
            Some("response") if msg["command"] == "launch" => {
                assert!(
                    msg["success"].as_bool().unwrap_or(false),
                    "launch failed: {msg}"
                );
                got_launch = true;
                println!("✓ launch response");
            }
            _ => println!("(other message, keep reading)"),
        }
    }
    // ---------------------------------------------------------------
    // 5. configurationDone (required after initialized)
    // ---------------------------------------------------------------
    send("configurationDone", json!({}))?;

    // let resp = read_message()?;
    // assert_eq!(resp["command"], "configurationDone");

    // ---------------------------------------------------------------
    // 6. Wait for the stopped event
    // ---------------------------------------------------------------
    loop {
        let msg = read_message()?;
        if msg["type"] == "event" && msg["event"] == "stopped" {
            println!("\n*** Program stopped ***");
            println!("Reason: {}", msg["body"]["reason"]);
            break;
        }
        // You may also receive output / process / thread events — just keep reading
    }

    // ---------------------------------------------------------------
    // 7. Clean disconnect
    // ---------------------------------------------------------------
    send("disconnect", json!({ "terminateDebuggee": true }))?;
    let _ = read_message(); // ignore response

    // Wait for the child to exit
    let status = child.wait()?;
    println!("lldb-dap exited: {}", status);

    Ok(())
}
