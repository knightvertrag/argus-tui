use std::collections::BTreeMap;
use std::path::PathBuf;

use crate::dap::types::{
    Breakpoint, BreakpointEvent, Capabilities, ContinuedEvent, Event, ExitedEvent, OutputEvent,
    ProcessEvent, Scope, StackFrame, StoppedEvent, Thread, ThreadEventReason, Variable,
};

/// A source breakpoint the session asked the adapter to install.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BreakpointSpec {
    pub line: i64,
    pub condition: Option<String>,
}

/// Spec we sent, plus the adapter's `Breakpoint` (id, verified, resolved line).
///
/// The adapter does not echo `condition`, so the spec is the only copy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundBreakpoint {
    pub spec: BreakpointSpec,
    pub breakpoint: Breakpoint,
}

/// Coarse execution phase the UI and agents should branch on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    Starting,
    Running,
    Stopped,
    Exited,
    Terminated,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Location {
    pub function: String,
    pub path: Option<PathBuf>,
    pub line: i64,
    pub column: i64,
}

#[derive(Debug, Clone)]
pub struct SessionState {
    pub phase: Phase,
    pub capabilities: Capabilities,
    pub threads: Vec<Thread>,
    pub focused_thread: Option<i64>,
    pub frames: Vec<StackFrame>,
    pub focused_frame: Option<i64>,
    pub scopes: Vec<Scope>,
    pub locals: Vec<Variable>,
    pub registers: Vec<Variable>,
    pub stopped: Option<StoppedEvent>,
    pub process: Option<ProcessEvent>,
    pub exit_code: Option<i64>,
    pub output: Vec<OutputEvent>,
    pub breakpoints: BTreeMap<PathBuf, Vec<BoundBreakpoint>>,
}

impl Default for SessionState {
    fn default() -> Self {
        Self {
            phase: Phase::Starting,
            capabilities: Capabilities::default(),
            threads: Vec::new(),
            focused_thread: None,
            frames: Vec::new(),
            focused_frame: None,
            scopes: Vec::new(),
            locals: Vec::new(),
            registers: Vec::new(),
            stopped: None,
            process: None,
            exit_code: None,
            output: Vec::new(),
            breakpoints: BTreeMap::new(),
        }
    }
}

impl SessionState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn location(&self) -> Option<Location> {
        let frame = self.current_frame()?;
        Some(Location {
            function: frame.name.clone(),
            path: frame
                .source
                .as_ref()
                .and_then(|source| source.path.as_ref())
                .map(PathBuf::from),
            line: frame.line,
            column: frame.column,
        })
    }

    pub fn current_frame(&self) -> Option<&StackFrame> {
        let id = self.focused_frame?;
        self.frames.iter().find(|frame| frame.id == id)
    }

    pub fn enter_running(&mut self) {
        if matches!(self.phase, Phase::Exited | Phase::Terminated) {
            return;
        }
        self.phase = Phase::Running;
        self.stopped = None;
        self.frames.clear();
        self.focused_frame = None;
        self.scopes.clear();
        self.locals.clear();
        self.registers.clear();
    }

    /// Apply a DAP event. Returns whether this was a `stopped` event.
    pub fn apply_event(&mut self, event: &Event) {
        let before = self.phase;
        match event {
            Event::Initialized => {}
            Event::Stopped(stopped) => {
                self.phase = Phase::Stopped;
                self.focused_thread = stopped.thread_id.or(self.focused_thread);
                self.stopped = Some(stopped.clone());
            }
            Event::Continued(ContinuedEvent {
                thread_id,
                all_threads_continued,
            }) => {
                if all_threads_continued.unwrap_or(true) || self.focused_thread == Some(*thread_id)
                {
                    self.enter_running();
                }
            }
            Event::Exited(ExitedEvent { exit_code }) => {
                self.phase = Phase::Exited;
                self.exit_code = Some(*exit_code);
            }
            Event::Terminated(_) => {
                self.phase = Phase::Terminated;
            }
            Event::Thread(thread) => match thread.reason {
                ThreadEventReason::Exited => {
                    self.threads
                        .retain(|existing| existing.id != thread.thread_id);
                    if self.focused_thread == Some(thread.thread_id) {
                        self.focused_thread = self.threads.first().map(|existing| existing.id);
                    }
                }
                ThreadEventReason::Started | ThreadEventReason::Other(_) => {
                    if !self
                        .threads
                        .iter()
                        .any(|existing| existing.id == thread.thread_id)
                    {
                        self.threads.push(Thread {
                            id: thread.thread_id,
                            name: format!("Thread {}", thread.thread_id),
                        });
                    }
                }
            },
            Event::Output(output) => self.output.push(output.clone()),
            Event::Process(process) => self.process = Some(process.clone()),
            Event::Breakpoint(BreakpointEvent { breakpoint, .. }) => {
                if let Some(id) = breakpoint.id {
                    for bps in self.breakpoints.values_mut() {
                        if let Some(existing) =
                            bps.iter_mut().find(|bp| bp.breakpoint.id == Some(id))
                        {
                            existing.breakpoint = breakpoint.clone();
                            break;
                        }
                    }
                }
            }
            Event::Unknown { .. } => {}
        }
        if self.phase != before {
            tracing::debug!(from = ?before, to = ?self.phase, "session phase");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dap::types::{Source, StoppedReason};

    fn stopped(thread_id: i64, reason: StoppedReason) -> Event {
        Event::Stopped(StoppedEvent {
            reason,
            description: None,
            thread_id: Some(thread_id),
            preserve_focus_hint: None,
            text: None,
            all_threads_stopped: Some(true),
            hit_breakpoint_ids: None,
        })
    }

    #[test]
    fn stopped_sets_phase_and_thread() {
        let mut state = SessionState::new();
        state.apply_event(&stopped(7, StoppedReason::Entry));
        assert_eq!(state.phase, Phase::Stopped);
        assert_eq!(state.focused_thread, Some(7));
        assert_eq!(state.stopped.as_ref().unwrap().reason, StoppedReason::Entry);
    }

    #[test]
    fn continue_clears_stack() {
        let mut state = SessionState::new();
        state.apply_event(&stopped(1, StoppedReason::Breakpoint));
        state.frames.push(StackFrame {
            id: 10,
            name: "main".into(),
            source: Some(Source {
                name: Some("hello.c".into()),
                path: Some("/tmp/hello.c".into()),
                source_reference: None,
            }),
            line: 12,
            column: 1,
            end_line: None,
            end_column: None,
            presentation_hint: None,
        });
        state.focused_frame = Some(10);
        state.apply_event(&Event::Continued(ContinuedEvent {
            thread_id: 1,
            all_threads_continued: Some(true),
        }));
        assert_eq!(state.phase, Phase::Running);
        assert!(state.frames.is_empty());
        assert!(state.stopped.is_none());
        assert_eq!(state.location(), None);
    }

    #[test]
    fn location_from_focused_frame() {
        let mut state = SessionState::new();
        state.frames.push(StackFrame {
            id: 3,
            name: "add".into(),
            source: Some(Source {
                name: Some("hello.c".into()),
                path: Some("/tmp/hello.c".into()),
                source_reference: None,
            }),
            line: 5,
            column: 9,
            end_line: None,
            end_column: None,
            presentation_hint: None,
        });
        state.focused_frame = Some(3);
        let loc = state.location().unwrap();
        assert_eq!(loc.function, "add");
        assert_eq!(loc.line, 5);
        assert_eq!(
            loc.path.as_deref(),
            Some(std::path::Path::new("/tmp/hello.c"))
        );
    }

    #[test]
    fn breakpoint_event_updates_verified_bit_and_keeps_condition() {
        let mut state = SessionState::new();
        let path = PathBuf::from("/tmp/main.c");
        state.breakpoints.insert(
            path.clone(),
            vec![BoundBreakpoint {
                spec: BreakpointSpec {
                    line: 8,
                    condition: Some("i == 10".into()),
                },
                breakpoint: Breakpoint {
                    id: Some(1),
                    verified: false,
                    message: None,
                    source: None,
                    line: Some(8),
                    column: None,
                },
            }],
        );
        state.apply_event(&Event::Breakpoint(BreakpointEvent {
            reason: crate::dap::types::BreakpointEventReason::Changed,
            breakpoint: Breakpoint {
                id: Some(1),
                verified: true,
                message: None,
                source: None,
                line: Some(8),
                column: None,
            },
        }));
        let bound = &state.breakpoints[&path][0];
        assert!(bound.breakpoint.verified);
        assert_eq!(bound.spec.condition.as_deref(), Some("i == 10"));
    }

    #[test]
    fn exited_wins_over_enter_running() {
        let mut state = SessionState::new();
        state.apply_event(&Event::Exited(ExitedEvent { exit_code: 0 }));
        state.enter_running();
        assert_eq!(state.phase, Phase::Exited);
        assert_eq!(state.exit_code, Some(0));
    }
}
