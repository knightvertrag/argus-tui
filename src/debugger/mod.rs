//! Debug session: domain state + high-level commands on top of [`crate::dap::Client`].

mod source;
mod state;

use std::path::PathBuf;
use std::process::ExitStatus;

use crate::dap::types::{
    ConfigurationDone, Continue, ContinueArguments, Disconnect, DisconnectArguments, Event,
    Initialize, InitializeArguments, Launch, LaunchArguments, Next, NextArguments, Request, Scopes,
    ScopesArguments, SetBreakpoints, SetBreakpointsArguments, Source, SourceBreakpoint, StackTrace,
    StackTraceArguments, StepIn, StepInArguments, StepOut, StepOutArguments, StoppedEvent, Threads,
    Variables, VariablesArguments,
};
use crate::dap::{Client, ClientError, Incoming};

#[allow(unused_imports)]
pub use source::CachedFile;
pub use source::SourceCache;
#[allow(unused_imports)]
pub use state::Location;
pub use state::{Phase, SessionState};

#[derive(Debug, thiserror::Error)]
pub enum SessionError {
    #[error(transparent)]
    Client(#[from] ClientError),
    #[error("debuggee ended before it stopped")]
    Ended,
    #[error("session is {actual:?}, expected {expected:?}")]
    WrongPhase { expected: Phase, actual: Phase },
    #[error("no focused thread")]
    #[allow(dead_code)]
    NoThread,
    #[error("no focused frame")]
    NoFrame,
    #[error(transparent)]
    Source(#[from] std::io::Error),
}

/// Domain-level notification produced from DAP events.
#[derive(Debug, Clone, PartialEq)]
pub enum SessionEvent {
    Initialized,
    Stopped(StoppedEvent),
    Continued(crate::dap::types::ContinuedEvent),
    Exited(crate::dap::types::ExitedEvent),
    Terminated,
    Thread(crate::dap::types::ThreadEvent),
    Output(crate::dap::types::OutputEvent),
    Process(crate::dap::types::ProcessEvent),
    Breakpoint(crate::dap::types::BreakpointEvent),
    Adapter { event: String },
    ReverseRequest { command: String },
}

#[derive(Debug, Clone)]
pub struct LaunchConfig {
    pub program: PathBuf,
    pub args: Vec<String>,
    pub cwd: Option<PathBuf>,
    pub stop_on_entry: bool,
    pub breakpoints: Vec<(PathBuf, Vec<i64>)>,
}

impl LaunchConfig {
    pub fn new(program: impl Into<PathBuf>) -> Self {
        Self {
            program: program.into(),
            args: Vec::new(),
            cwd: None,
            stop_on_entry: false,
            breakpoints: Vec::new(),
        }
    }
}

pub struct Session {
    client: Client,
    state: SessionState,
    sources: SourceCache,
}

#[allow(dead_code)]
impl Session {
    /// Spawn `lldb-dap`, initialize, launch, optional breakpoints, configurationDone.
    pub async fn launch(config: LaunchConfig) -> Result<Session, SessionError> {
        tracing::info!(
            program = %config.program.display(),
            stop_on_entry = config.stop_on_entry,
            "launching session"
        );
        let client = Client::spawn()?;
        let mut session = Session {
            client,
            state: SessionState::new(),
            sources: SourceCache::new(),
        };

        session.state.capabilities = session
            .request::<Initialize>(InitializeArguments::new("lldb-dap"))
            .await?;

        let mut launch = LaunchArguments::new(config.program.to_string_lossy());
        if !config.args.is_empty() {
            launch.args = Some(config.args);
        }
        launch.cwd = config
            .cwd
            .as_ref()
            .map(|cwd| cwd.to_string_lossy().into_owned());
        launch.stop_on_entry = Some(config.stop_on_entry);
        session.request::<Launch>(launch).await?;
        session.wait_for_initialized().await?;

        for (path, lines) in config.breakpoints {
            session.set_breakpoints(path, &lines).await?;
        }

        if session
            .state
            .capabilities
            .supports_configuration_done_request
        {
            session.request::<ConfigurationDone>(()).await?;
        }

        if !matches!(
            session.state.phase,
            Phase::Stopped | Phase::Exited | Phase::Terminated
        ) {
            session.state.phase = Phase::Running;
        }

        Ok(session)
    }

    pub fn state(&self) -> &SessionState {
        &self.state
    }

    pub fn sources(&mut self) -> &mut SourceCache {
        &mut self.sources
    }

    /// Next DAP-derived event, applying it to [`SessionState`].
    pub async fn next_event(&mut self) -> Result<SessionEvent, SessionError> {
        let incoming = self.client.recv().await?;
        Ok(self.handle_incoming(incoming))
    }

    /// Block until a `stopped` event (or fail if the debuggee exits first).
    /// Then fetches threads + stack for the focused thread.
    pub async fn wait_until_stopped(&mut self) -> Result<&StoppedEvent, SessionError> {
        if self.state.phase != Phase::Stopped {
            loop {
                match self.next_event().await? {
                    SessionEvent::Stopped(_) => break,
                    SessionEvent::Exited(_) | SessionEvent::Terminated => {
                        return Err(SessionError::Ended);
                    }
                    _ => {}
                }
            }
        }
        self.refresh_stop_context().await?;
        Ok(self
            .state
            .stopped
            .as_ref()
            .expect("stopped phase has stopped event"))
    }

    /// DAP `continue`.
    pub async fn resume(&mut self) -> Result<(), SessionError> {
        tracing::debug!("resume");
        self.step_request::<Continue>(ContinueArguments {
            thread_id: self.require_thread()?,
            single_thread: None,
        })
        .await
    }

    /// DAP `next` (step over).
    pub async fn step_over(&mut self) -> Result<(), SessionError> {
        tracing::debug!("step_over");
        self.step_request::<Next>(NextArguments {
            thread_id: self.require_thread()?,
            single_thread: None,
            granularity: None,
        })
        .await
    }

    /// DAP `stepIn`.
    pub async fn step_in(&mut self) -> Result<(), SessionError> {
        tracing::debug!("step_in");
        self.step_request::<StepIn>(StepInArguments {
            thread_id: self.require_thread()?,
            single_thread: None,
            target_id: None,
            granularity: None,
        })
        .await
    }

    /// DAP `stepOut`.
    pub async fn step_out(&mut self) -> Result<(), SessionError> {
        tracing::debug!("step_out");
        self.step_request::<StepOut>(StepOutArguments {
            thread_id: self.require_thread()?,
            single_thread: None,
            granularity: None,
        })
        .await
    }

    pub async fn set_breakpoints(
        &mut self,
        path: PathBuf,
        lines: &[i64],
    ) -> Result<Vec<crate::dap::types::Breakpoint>, SessionError> {
        tracing::debug!(path = %path.display(), ?lines, "set_breakpoints");
        let source = Source {
            name: path
                .file_name()
                .map(|name| name.to_string_lossy().into_owned()),
            path: Some(path.to_string_lossy().into_owned()),
            source_reference: None,
        };
        let breakpoints = lines
            .iter()
            .map(|line| SourceBreakpoint {
                line: *line,
                column: None,
                condition: None,
                hit_condition: None,
            })
            .collect();
        let response = self
            .request::<SetBreakpoints>(SetBreakpointsArguments {
                source,
                breakpoints: Some(breakpoints),
                source_modified: None,
            })
            .await?;
        for breakpoint in &response.breakpoints {
            if !breakpoint.verified {
                tracing::warn!(
                    path = %path.display(),
                    line = ?breakpoint.line,
                    message = ?breakpoint.message,
                    "breakpoint not verified"
                );
            }
        }
        self.state
            .breakpoints
            .insert(path, response.breakpoints.clone());
        Ok(response.breakpoints)
    }

    /// Threads + stack frames for the focused (or first) thread.
    pub async fn refresh_stop_context(&mut self) -> Result<(), SessionError> {
        tracing::debug!("refresh_stop_context");
        self.require_phase(Phase::Stopped)?;
        let threads = self.request::<Threads>(()).await?;
        self.state.threads = threads.threads;
        if self.state.focused_thread.is_none() {
            self.state.focused_thread = self.state.threads.first().map(|thread| thread.id);
        }

        let Some(thread_id) = self.state.focused_thread else {
            self.state.frames.clear();
            self.state.focused_frame = None;
            return Ok(());
        };

        let stack = self
            .request::<StackTrace>(StackTraceArguments {
                thread_id,
                start_frame: Some(0),
                levels: Some(20),
            })
            .await?;
        self.state.frames = stack.stack_frames;
        self.state.focused_frame = self.state.frames.first().map(|frame| frame.id);
        self.state.scopes.clear();
        self.state.locals.clear();
        Ok(())
    }

    pub async fn select_frame(&mut self, frame_id: i64) -> Result<(), SessionError> {
        self.require_phase(Phase::Stopped)?;
        if !self.state.frames.iter().any(|frame| frame.id == frame_id) {
            return Err(SessionError::NoFrame);
        }
        self.state.focused_frame = Some(frame_id);
        self.state.scopes.clear();
        self.state.locals.clear();
        Ok(())
    }

    /// Scopes + non-expensive variables for the focused frame.
    pub async fn load_locals(&mut self) -> Result<&[crate::dap::types::Variable], SessionError> {
        tracing::debug!("load_locals");
        self.require_phase(Phase::Stopped)?;
        let frame_id = self.state.focused_frame.ok_or(SessionError::NoFrame)?;
        let scopes = self.request::<Scopes>(ScopesArguments { frame_id }).await?;
        self.state.scopes = scopes.scopes;

        let mut locals = Vec::new();
        let references: Vec<i64> = self
            .state
            .scopes
            .iter()
            .filter(|scope| !scope.expensive)
            .map(|scope| scope.variables_reference)
            .filter(|reference| *reference > 0)
            .collect();
        for variables_reference in references {
            let response = self
                .request::<Variables>(VariablesArguments {
                    variables_reference,
                    filter: None,
                    start: None,
                    count: None,
                })
                .await?;
            locals.extend(response.variables);
        }
        self.state.locals = locals;
        Ok(&self.state.locals)
    }

    pub async fn disconnect(mut self) -> Result<ExitStatus, SessionError> {
        let _ = self
            .request::<Disconnect>(DisconnectArguments {
                terminate_debuggee: Some(true),
                ..Default::default()
            })
            .await;
        Ok(self.client.shutdown().await?)
    }

    async fn wait_for_initialized(&mut self) -> Result<(), SessionError> {
        loop {
            match self.next_event().await? {
                SessionEvent::Initialized => return Ok(()),
                SessionEvent::Exited(_) | SessionEvent::Terminated => {
                    return Err(SessionError::Ended);
                }
                _ => {}
            }
        }
    }

    async fn step_request<R: Request>(&mut self, args: R::Arguments) -> Result<(), SessionError> {
        self.require_phase(Phase::Stopped)?;
        self.state.enter_running();
        tracing::info!(phase = ?self.state.phase, "session running");
        self.request::<R>(args).await?;
        Ok(())
    }

    async fn request<R: Request>(
        &mut self,
        args: R::Arguments,
    ) -> Result<R::Response, SessionError> {
        let response = self.client.request::<R>(args).await?;
        self.apply_inbox();
        Ok(response)
    }

    fn apply_inbox(&mut self) {
        let incoming = self.client.drain_inbox();
        for item in incoming {
            self.handle_incoming(item);
        }
    }

    fn handle_incoming(&mut self, incoming: Incoming) -> SessionEvent {
        match incoming {
            Incoming::Event(event) => {
                self.state.apply_event(&event);
                match event {
                    Event::Initialized => SessionEvent::Initialized,
                    Event::Stopped(stopped) => {
                        tracing::info!(
                            reason = ?stopped.reason,
                            thread_id = ?stopped.thread_id,
                            "session stopped"
                        );
                        SessionEvent::Stopped(stopped)
                    }
                    Event::Continued(continued) => SessionEvent::Continued(continued),
                    Event::Exited(exited) => {
                        tracing::info!(exit_code = exited.exit_code, "session exited");
                        SessionEvent::Exited(exited)
                    }
                    Event::Terminated(_) => {
                        tracing::info!("session terminated");
                        SessionEvent::Terminated
                    }
                    Event::Thread(thread) => SessionEvent::Thread(thread),
                    Event::Output(output) => SessionEvent::Output(output),
                    Event::Process(process) => SessionEvent::Process(process),
                    Event::Breakpoint(breakpoint) => SessionEvent::Breakpoint(breakpoint),
                    Event::Unknown { event, .. } => {
                        tracing::debug!(event, "unknown adapter event");
                        SessionEvent::Adapter { event }
                    }
                }
            }
            Incoming::ReverseRequest(request) => {
                tracing::warn!(command = %request.command, "reverse request ignored");
                SessionEvent::ReverseRequest {
                    command: request.command,
                }
            }
        }
    }

    fn require_phase(&self, expected: Phase) -> Result<(), SessionError> {
        if self.state.phase == expected {
            Ok(())
        } else {
            Err(SessionError::WrongPhase {
                expected,
                actual: self.state.phase,
            })
        }
    }

    fn require_thread(&self) -> Result<i64, SessionError> {
        self.require_phase(Phase::Stopped)?;
        self.state.focused_thread.ok_or(SessionError::NoThread)
    }
}
