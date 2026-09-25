mod command;
mod dap;
mod debugger;
mod driver;
mod event;
mod highlight;
mod logging;
mod ui;
mod view;

use std::path::PathBuf;

use anyhow::{Context, Result, bail};
use tokio::sync::{mpsc, watch};

use debugger::{LaunchConfig, Session};
use event::EventHandler;
use ui::UiState;
use view::ViewModel;

#[tokio::main]
async fn main() -> Result<()> {
    let mode = parse_args(std::env::args().skip(1)).map_err(|err| anyhow::anyhow!(err))?;
    match mode {
        Mode::Harness => harness().await,
        Mode::Tui(config) => run_tui(config).await,
    }
}

async fn run_tui(mut config: LaunchConfig) -> Result<()> {
    if !config.program.exists() {
        bail!("program not found: {}", config.program.display());
    }
    config.program = config.program.canonicalize()?;
    // Leave breakpoint paths as given. lldb matches them against the compile
    // path in DWARF, and canonicalize can change casing on a case-insensitive
    // volume (`Code` vs `COde`) so the breakpoint is never verified.

    logging::init_file().context("failed to open argus-tui.log")?;
    let mut terminal = ratatui::try_init().context("failed to init terminal")?;
    let result = session_ui(&mut terminal, config).await;
    ratatui::restore();
    result
}

async fn session_ui(terminal: &mut ratatui::DefaultTerminal, config: LaunchConfig) -> Result<()> {
    let (cmd_tx, cmd_rx) = mpsc::unbounded_channel();
    let (view_tx, mut view_rx) = watch::channel(ViewModel::starting());
    let driver = tokio::spawn(driver::run(config, cmd_rx, view_tx));
    tokio::pin!(driver);

    let mut events = EventHandler::new();
    let mut ui = UiState::default();
    let mut model = view_rx.borrow_and_update().clone();

    loop {
        terminal.draw(|frame| ui::draw(frame, &model, &mut ui))?;
        tokio::select! {
            biased;
            result = &mut driver => {
                result.context("driver task")??;
                break;
            }
            event = events.next() => {
                match event {
                    Some(crossterm::event::Event::Key(key)) => {
                        ui::handle_key(key, &mut ui, &cmd_tx);
                    }
                    Some(_) => {}
                    None => {
                        let _ = cmd_tx.send(command::Command::Quit);
                    }
                }
            }
            changed = view_rx.changed() => {
                if changed.is_err() {
                    continue;
                }
                let next = view_rx.borrow().clone();
                if next.line != model.line || next.phase != model.phase {
                    ui.code_stick = false;
                    ui.list_stick = false;
                }
                model = next;
            }
        }
    }
    Ok(())
}

async fn harness() -> Result<()> {
    logging::init_stderr();

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

enum Mode {
    Harness,
    Tui(LaunchConfig),
}

fn parse_args<I, S>(args: I) -> Result<Mode, String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let mut harness = false;
    let mut stop_on_entry = false;
    let mut breakpoints: Vec<(PathBuf, Vec<i64>)> = Vec::new();
    let mut program: Option<PathBuf> = None;
    let mut program_args = Vec::new();
    let mut positional = false;
    let mut pending_break = false;

    for arg in args {
        let arg = arg.as_ref();
        if pending_break {
            push_breakpoint(&mut breakpoints, arg)?;
            pending_break = false;
            continue;
        }
        if positional {
            if program.is_none() {
                program = Some(PathBuf::from(arg));
            } else {
                program_args.push(arg.to_string());
            }
            continue;
        }
        match arg {
            "--harness" => harness = true,
            "--stop-on-entry" => stop_on_entry = true,
            "--" => positional = true,
            "-b" => pending_break = true,
            other if other.starts_with('-') => {
                return Err(format!("unknown flag {other}"));
            }
            other => {
                program = Some(PathBuf::from(other));
                positional = true;
            }
        }
    }

    if pending_break {
        return Err("break needs file:line".into());
    }
    if harness {
        return Ok(Mode::Harness);
    }
    let Some(program) = program else {
        return Err(
            "usage: argus-tui [--stop-on-entry] [-b file:line]... <program> [args...]".into(),
        );
    };
    let mut config = LaunchConfig::new(program);
    config.stop_on_entry = stop_on_entry;
    config.args = program_args;
    config.breakpoints = breakpoints;
    Ok(Mode::Tui(config))
}

fn push_breakpoint(breakpoints: &mut Vec<(PathBuf, Vec<i64>)>, spec: &str) -> Result<(), String> {
    let (file, line) = spec
        .rsplit_once(':')
        .ok_or_else(|| "expected file:line".to_string())?;
    if file.is_empty() {
        return Err("expected file:line".into());
    }
    let line: i64 = line
        .parse()
        .map_err(|_| "expected a line number".to_string())?;
    if line < 1 {
        return Err("line numbers start at 1".into());
    }
    let path = PathBuf::from(file);
    if let Some((_, lines)) = breakpoints
        .iter_mut()
        .find(|(existing, _)| existing == &path)
    {
        lines.push(line);
    } else {
        breakpoints.push((path, vec![line]));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn groups_breakpoints_and_program_args() {
        let mode = parse_args([
            "-b",
            "a.c:3",
            "--stop-on-entry",
            "-b",
            "a.c:5",
            "-b",
            "b.c:1",
            "prog",
            "one",
        ])
        .unwrap();
        let Mode::Tui(config) = mode else {
            panic!("expected tui");
        };
        assert!(config.stop_on_entry);
        assert_eq!(config.program, PathBuf::from("prog"));
        assert_eq!(config.args, vec!["one".to_string()]);
        assert_eq!(
            config.breakpoints,
            vec![
                (PathBuf::from("a.c"), vec![3, 5]),
                (PathBuf::from("b.c"), vec![1]),
            ]
        );
    }

    #[test]
    fn harness_flag_and_missing_program() {
        assert!(matches!(parse_args(["--harness"]).unwrap(), Mode::Harness));
        assert!(parse_args(Vec::<String>::new()).is_err());
    }
}
