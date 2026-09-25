//! Owns the debug session. The draw loop only sends [`Command`]s and paints [`ViewModel`]s.

use std::path::{Path, PathBuf};

use tokio::sync::{mpsc, watch};

use crate::command::Command;
use crate::dap::types::OutputCategory;
use crate::debugger::{BreakpointSpec, LaunchConfig, Phase, Session, SessionError, SessionEvent};
use crate::view::{self, TermLine, TermStyle, ViewModel, Watch};

const TERMINAL_CAP: usize = 500;

pub async fn run(
    config: LaunchConfig,
    mut cmds: mpsc::UnboundedReceiver<Command>,
    tx: watch::Sender<ViewModel>,
) -> Result<(), SessionError> {
    let launch = Session::launch(config);
    tokio::pin!(launch);
    let session = loop {
        tokio::select! {
            biased;
            cmd = cmds.recv() => {
                match cmd {
                    Some(Command::Quit) | None => return Ok(()),
                    Some(_) => {
                        let mut model = ViewModel::starting();
                        model.status = "still launching".into();
                        let _ = tx.send(model);
                    }
                }
            }
            result = &mut launch => {
                match result {
                    Ok(session) => break session,
                    Err(err) => {
                        let mut model = ViewModel::starting();
                        model.status = err.to_string();
                        model.terminal.push(TermLine {
                            style: TermStyle::Error,
                            text: err.to_string(),
                        });
                        let _ = tx.send(model);
                        wait_for_quit(&mut cmds).await;
                        return Ok(());
                    }
                }
            }
        }
    };

    let mut app = App {
        session: Some(session),
        watches: Vec::new(),
        terminal: Vec::new(),
        output_seen: 0,
        status: String::new(),
        tx,
    };
    if app.session().state().phase == Phase::Stopped {
        app.on_stopped().await;
    } else {
        app.publish();
    }

    loop {
        if matches!(
            app.session().state().phase,
            Phase::Exited | Phase::Terminated
        ) {
            match cmds.recv().await {
                Some(Command::Quit) | None => return app.shutdown().await,
                Some(cmd) => app.dispatch(cmd).await,
            }
            continue;
        }

        tokio::select! {
            biased;
            cmd = cmds.recv() => {
                match cmd {
                    Some(Command::Quit) | None => return app.shutdown().await,
                    Some(cmd) => app.dispatch(cmd).await,
                }
            }
            event = app.session().next_event() => {
                match event {
                    Ok(event) => app.on_event(event).await,
                    Err(err) => {
                        app.fail(&err);
                        wait_for_quit(&mut cmds).await;
                        return app.shutdown().await;
                    }
                }
            }
        }
    }
}

struct App {
    session: Option<Session>,
    watches: Vec<Watch>,
    terminal: Vec<TermLine>,
    output_seen: usize,
    status: String,
    tx: watch::Sender<ViewModel>,
}

impl App {
    fn session(&mut self) -> &mut Session {
        self.session.as_mut().expect("session is live")
    }

    async fn shutdown(mut self) -> Result<(), SessionError> {
        if let Some(session) = self.session.take() {
            session.disconnect().await?;
        }
        Ok(())
    }

    fn publish(&mut self) {
        self.take_output();
        let watches = self.watches.clone();
        let terminal = self.terminal.clone();
        let status = self.status.clone();
        let model = ViewModel::capture(self.session(), &watches, &terminal, &status);
        let _ = self.tx.send(model);
    }

    fn take_output(&mut self) {
        let seen = self.output_seen;
        let fresh: Vec<_> = self
            .session()
            .state()
            .output
            .iter()
            .skip(seen)
            .cloned()
            .collect();
        self.output_seen += fresh.len();
        for event in fresh {
            let style = match event.category {
                Some(OutputCategory::Stderr) => TermStyle::Err,
                _ => TermStyle::Out,
            };
            for line in event.output.lines() {
                self.push_line(TermLine {
                    style,
                    text: line.to_string(),
                });
            }
        }
    }

    fn push_line(&mut self, line: TermLine) {
        self.terminal.push(line);
        if self.terminal.len() > TERMINAL_CAP {
            let extra = self.terminal.len() - TERMINAL_CAP;
            self.terminal.drain(0..extra);
        }
    }

    fn echo(&mut self, text: &str) {
        self.push_line(TermLine {
            style: TermStyle::Echo,
            text: format!("> {text}"),
        });
    }

    fn fail(&mut self, err: &SessionError) {
        tracing::warn!(error = %err, "session command failed");
        self.status = err.to_string();
        self.push_line(TermLine {
            style: TermStyle::Error,
            text: err.to_string(),
        });
        self.publish();
    }

    async fn on_event(&mut self, event: SessionEvent) {
        match event {
            SessionEvent::Stopped(_) => self.on_stopped().await,
            SessionEvent::Exited(event) => {
                self.status = format!("exited {}", event.exit_code);
                self.publish();
            }
            SessionEvent::Terminated => {
                self.status = "terminated".into();
                self.publish();
            }
            SessionEvent::Initialized
            | SessionEvent::Continued(_)
            | SessionEvent::Thread(_)
            | SessionEvent::Output(_)
            | SessionEvent::Process(_)
            | SessionEvent::Breakpoint(_)
            | SessionEvent::Adapter { .. }
            | SessionEvent::ReverseRequest { .. } => self.publish(),
        }
    }

    async fn on_stopped(&mut self) {
        self.status.clear();
        if let Err(err) = self.session().refresh_stop_context().await {
            self.fail(&err);
            return;
        }
        if let Err(err) = self.reload_variables().await {
            self.fail(&err);
            return;
        }
        self.publish();
    }

    async fn reload_variables(&mut self) -> Result<(), SessionError> {
        self.session().load_frame_variables().await?;
        self.eval_watches().await;
        Ok(())
    }

    async fn eval_watches(&mut self) {
        if self.session().state().phase != Phase::Stopped {
            return;
        }
        let expressions: Vec<String> = self
            .watches
            .iter()
            .map(|watch| watch.expression.clone())
            .collect();
        let mut values = Vec::with_capacity(expressions.len());
        for expression in &expressions {
            let value = match self.session().evaluate(expression).await {
                Ok(value) => value,
                Err(err) => err.to_string(),
            };
            values.push(value);
        }
        for (watch, value) in self.watches.iter_mut().zip(values) {
            watch.value = value;
        }
    }

    async fn dispatch(&mut self, command: Command) {
        match command {
            Command::Help | Command::Quit => {}
            Command::Resume | Command::StepOver | Command::StepIn | Command::StepOut => {
                self.step(command).await;
            }
            Command::ToggleBreakpoint => self.toggle_breakpoint().await,
            Command::Break {
                path,
                line,
                condition,
            } => self.add_breakpoint(path, line, condition).await,
            Command::DeleteBreakpoint(id) => self.delete_breakpoint(id).await,
            Command::Watch(expression) => self.add_watch(expression).await,
            Command::Unwatch(index) => self.unwatch(index),
            Command::Thread(id) => self.focus_thread(id).await,
            Command::Frame(index) => self.focus_frame(index).await,
        }
    }

    fn stopped(&mut self) -> bool {
        if self.session().state().phase == Phase::Stopped {
            true
        } else {
            self.status = "not stopped".into();
            self.publish();
            false
        }
    }

    async fn step(&mut self, command: Command) {
        if !self.stopped() {
            return;
        }
        self.echo(&command_label(&command));
        let result = match command {
            Command::Resume => self.session().resume().await,
            Command::StepOver => self.session().step_over().await,
            Command::StepIn => self.session().step_in().await,
            Command::StepOut => self.session().step_out().await,
            _ => return,
        };
        if let Err(err) = result {
            self.fail(&err);
            return;
        }
        if self.session().state().phase == Phase::Stopped {
            self.on_stopped().await;
        } else {
            self.status.clear();
            self.publish();
        }
    }

    async fn toggle_breakpoint(&mut self) {
        if !self.stopped() {
            return;
        }
        let Some(path) = self.session().state().location().and_then(|loc| loc.path) else {
            self.status = "no source".into();
            self.publish();
            return;
        };
        let Some(line) = self.session().state().location().map(|loc| loc.line) else {
            self.status = "no source".into();
            self.publish();
            return;
        };
        let key = self.key_for(&path);
        let mut specs = self.specs_for(&key);
        if let Some(index) = specs.iter().position(|spec| spec.line == line) {
            specs.remove(index);
            self.echo(&format!("delete {}", file_line(&key, line)));
        } else {
            specs.push(BreakpointSpec {
                line,
                condition: None,
            });
            self.echo(&format!("break {}", file_line(&key, line)));
        }
        self.install(key, specs).await;
    }

    async fn add_breakpoint(&mut self, path: PathBuf, line: i64, condition: Option<String>) {
        if !self.stopped() {
            return;
        }
        self.echo(&match &condition {
            Some(condition) => format!("break {}:{} if {condition}", path.display(), line),
            None => format!("break {}:{}", path.display(), line),
        });
        let key = self.key_for(&path);
        let mut specs = self.specs_for(&key);
        if let Some(spec) = specs.iter_mut().find(|spec| spec.line == line) {
            spec.condition = condition;
        } else {
            specs.push(BreakpointSpec { line, condition });
        }
        self.install(key, specs).await;
    }

    async fn delete_breakpoint(&mut self, id: i64) {
        if !self.stopped() {
            return;
        }
        let found = self
            .session()
            .state()
            .breakpoints
            .iter()
            .find_map(|(path, bps)| {
                bps.iter()
                    .any(|bp| bp.breakpoint.id == Some(id))
                    .then(|| path.clone())
            });
        let Some(path) = found else {
            self.status = format!("no breakpoint {id}");
            self.publish();
            return;
        };
        self.echo(&format!("delete {id}"));
        let specs = self
            .session()
            .state()
            .breakpoints
            .get(&path)
            .map(|bps| {
                bps.iter()
                    .filter(|bp| bp.breakpoint.id != Some(id))
                    .map(|bp| bp.spec.clone())
                    .collect()
            })
            .unwrap_or_default();
        self.install(path, specs).await;
    }

    async fn install(&mut self, path: PathBuf, specs: Vec<BreakpointSpec>) {
        if let Err(err) = self.session().set_breakpoints(path, &specs).await {
            self.fail(&err);
        } else {
            self.status.clear();
            self.publish();
        }
    }

    fn key_for(&mut self, path: &Path) -> PathBuf {
        self.session()
            .state()
            .breakpoints
            .keys()
            .find(|key| view::same_file(key, path))
            .cloned()
            .unwrap_or_else(|| path.to_path_buf())
    }

    fn specs_for(&mut self, key: &Path) -> Vec<BreakpointSpec> {
        self.session()
            .state()
            .breakpoints
            .get(key)
            .map(|bps| bps.iter().map(|bp| bp.spec.clone()).collect())
            .unwrap_or_default()
    }

    async fn add_watch(&mut self, expression: String) {
        self.echo(&format!("watch {expression}"));
        self.watches.push(Watch {
            expression: expression.clone(),
            value: String::new(),
        });
        if self.session().state().phase == Phase::Stopped {
            let value = match self.session().evaluate(&expression).await {
                Ok(value) => value,
                Err(err) => err.to_string(),
            };
            if let Some(watch) = self.watches.last_mut() {
                watch.value = value;
            }
        }
        self.publish();
    }

    fn unwatch(&mut self, index: usize) {
        if index == 0 || index > self.watches.len() {
            self.status = format!("no watch {index}");
            self.publish();
            return;
        }
        self.watches.remove(index - 1);
        self.echo(&format!("unwatch {index}"));
        self.publish();
    }

    async fn focus_thread(&mut self, id: i64) {
        if !self.stopped() {
            return;
        }
        self.echo(&format!("thread {id}"));
        if let Err(err) = self.session().select_thread(id).await {
            self.fail(&err);
            return;
        }
        if let Err(err) = self.reload_variables().await {
            self.fail(&err);
            return;
        }
        self.status.clear();
        self.publish();
    }

    async fn focus_frame(&mut self, index: usize) {
        if !self.stopped() {
            return;
        }
        let Some(id) = self
            .session()
            .state()
            .frames
            .get(index)
            .map(|frame| frame.id)
        else {
            self.status = format!("no frame {index}");
            self.publish();
            return;
        };
        self.echo(&format!("frame {index}"));
        if let Err(err) = self.session().select_frame(id).await {
            self.fail(&err);
            return;
        }
        if let Err(err) = self.reload_variables().await {
            self.fail(&err);
            return;
        }
        self.status.clear();
        self.publish();
    }
}

fn command_label(command: &Command) -> String {
    match command {
        Command::Resume => "continue".into(),
        Command::StepOver => "next".into(),
        Command::StepIn => "step".into(),
        Command::StepOut => "finish".into(),
        _ => "command".into(),
    }
}

fn file_line(path: &Path, line: i64) -> String {
    format!("{}:{line}", path.display())
}

async fn wait_for_quit(cmds: &mut mpsc::UnboundedReceiver<Command>) {
    while let Some(command) = cmds.recv().await {
        if matches!(command, Command::Quit) {
            break;
        }
    }
}
