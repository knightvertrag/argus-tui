//! Snapshot the draw loop paints. Built from session state; the widgets never call Session.

use std::path::Path;

use crate::debugger::{Phase, Session};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Watch {
    pub expression: String,
    pub value: String,
}

/// One local, split so the value can be colored apart from the type and name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalRow {
    pub head: String,
    pub value: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BreakpointRow {
    pub id: Option<i64>,
    pub location: String,
    pub condition: String,
    pub verified: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegisterRow {
    pub name: String,
    pub value: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThreadRow {
    pub id: i64,
    pub name: String,
    pub focused: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FrameRow {
    pub label: String,
    pub focused: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TermStyle {
    Out,
    Err,
    Echo,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TermLine {
    pub style: TermStyle,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ViewModel {
    pub phase: Phase,
    pub exit_code: Option<i64>,
    pub function: Option<String>,
    pub line: Option<i64>,
    pub column: Option<i64>,
    pub has_source: bool,
    pub file_name: Option<String>,
    pub source_lines: Vec<String>,
    pub locals: Vec<LocalRow>,
    pub source_error: Option<String>,
    pub breakpoint_lines: Vec<i64>,
    pub watches: Vec<Watch>,
    pub breakpoints: Vec<BreakpointRow>,
    pub registers: Vec<RegisterRow>,
    pub registers_unavailable: bool,
    pub threads: Vec<ThreadRow>,
    pub frames: Vec<FrameRow>,
    pub terminal: Vec<TermLine>,
    pub status: String,
}

impl ViewModel {
    pub fn starting() -> Self {
        Self {
            phase: Phase::Starting,
            exit_code: None,
            function: None,
            line: None,
            column: None,
            has_source: false,
            file_name: None,
            source_lines: Vec::new(),
            locals: Vec::new(),
            source_error: None,
            breakpoint_lines: Vec::new(),
            watches: Vec::new(),
            breakpoints: Vec::new(),
            registers: Vec::new(),
            registers_unavailable: false,
            threads: Vec::new(),
            frames: Vec::new(),
            terminal: Vec::new(),
            status: "launching".into(),
        }
    }

    pub fn capture(
        session: &mut Session,
        watches: &[Watch],
        terminal: &[TermLine],
        status: &str,
    ) -> Self {
        let snap = {
            let state = session.state();
            let location = state.location();
            let path = location.as_ref().and_then(|loc| loc.path.clone());
            let function = location.as_ref().map(|loc| loc.function.clone());
            let line = location.as_ref().map(|loc| loc.line);
            let column = location.as_ref().map(|loc| loc.column);
            let breakpoint_lines = state
                .breakpoints
                .iter()
                .filter(|(bp_path, _)| {
                    path.as_ref()
                        .is_some_and(|current| same_file(current, bp_path))
                })
                .flat_map(|(_, bps)| bps.iter().map(|bp| bp.spec.line))
                .collect();
            let mut breakpoints = Vec::new();
            for (bp_path, bps) in &state.breakpoints {
                for bound in bps {
                    let line = bound.breakpoint.line.unwrap_or(bound.spec.line);
                    let file = bound
                        .breakpoint
                        .source
                        .as_ref()
                        .and_then(|source| source.path.as_deref())
                        .map(Path::new)
                        .unwrap_or(bp_path.as_path());
                    breakpoints.push(BreakpointRow {
                        id: bound.breakpoint.id,
                        location: format!("{}:{line}", file_label(file)),
                        condition: bound.spec.condition.clone().unwrap_or_default(),
                        verified: bound.breakpoint.verified,
                    });
                }
            }
            let locals = state
                .locals
                .iter()
                .map(|variable| {
                    let head = match &variable.type_field {
                        Some(ty) => format!("{ty} {} = ", variable.name),
                        None => format!("{} = ", variable.name),
                    };
                    LocalRow {
                        head,
                        value: variable.value.clone(),
                    }
                })
                .collect();
            let registers = state
                .registers
                .iter()
                .map(|variable| RegisterRow {
                    name: variable.name.clone(),
                    value: variable.value.clone(),
                })
                .collect();
            let registers_unavailable = state.phase == Phase::Stopped
                && state.focused_frame.is_some()
                && !state.scopes.is_empty()
                && !state.scopes.iter().any(|scope| scope.name == "Registers");
            let threads = state
                .threads
                .iter()
                .map(|thread| ThreadRow {
                    id: thread.id,
                    name: thread.name.clone(),
                    focused: state.focused_thread == Some(thread.id),
                })
                .collect();
            let frames = state
                .frames
                .iter()
                .enumerate()
                .map(|(index, frame)| {
                    let place = frame.source.as_ref().and_then(|source| {
                        source
                            .path
                            .as_deref()
                            .and_then(crate::debugger::SessionState::source_file_path)
                            .or(source.name.as_deref())
                    });
                    let label = match place {
                        Some(path) => format!(
                            "#{index} {} at {}:{}",
                            frame.name,
                            file_label(Path::new(path)),
                            frame.line
                        ),
                        None => format!("#{index} {}", frame.name),
                    };
                    FrameRow {
                        label,
                        focused: state.focused_frame == Some(frame.id),
                    }
                })
                .collect();
            (
                state.phase,
                state.exit_code,
                function,
                line,
                column,
                path,
                breakpoint_lines,
                breakpoints,
                locals,
                registers,
                registers_unavailable,
                threads,
                frames,
            )
        };
        let (
            phase,
            exit_code,
            function,
            line,
            column,
            path,
            breakpoint_lines,
            breakpoints,
            locals,
            registers,
            registers_unavailable,
            threads,
            frames,
        ) = snap;

        let mut source_lines = Vec::new();
        let mut source_error = None;
        if let Some(path) = &path {
            match session.sources().load(path) {
                Ok(file) => source_lines = file.lines.clone(),
                Err(err) => {
                    source_error = Some(format!("could not read {}: {err}", path.display()));
                }
            }
        }

        Self {
            phase,
            exit_code,
            function,
            line,
            column,
            has_source: path.is_some() && source_error.is_none(),
            file_name: path.as_ref().and_then(|path| {
                path.file_name()
                    .map(|name| name.to_string_lossy().into_owned())
            }),
            source_lines,
            locals,
            source_error,
            breakpoint_lines,
            watches: watches.to_vec(),
            breakpoints,
            registers,
            registers_unavailable,
            threads,
            frames,
            terminal: terminal.to_vec(),
            status: status.to_string(),
        }
    }
}

pub fn same_file(a: &Path, b: &Path) -> bool {
    if a == b {
        return true;
    }
    if let (Ok(a), Ok(b)) = (a.canonicalize(), b.canonicalize()) {
        return a == b;
    }
    a.file_name().is_some() && a.file_name() == b.file_name()
}

fn file_label(path: &Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.display().to_string())
}

/// Footer inside the code pane.
pub fn code_footer(model: &ViewModel) -> String {
    match model.phase {
        Phase::Running => "Running".into(),
        Phase::Starting => {
            if model.status.is_empty() {
                "Starting".into()
            } else {
                model.status.clone()
            }
        }
        Phase::Exited => format!("Exited ({})", model.exit_code.unwrap_or(0)),
        Phase::Terminated => "Terminated".into(),
        Phase::Stopped => match (&model.line, &model.column, &model.function) {
            (Some(line), Some(column), Some(function)) if model.has_source => {
                format!("Ln {line}, Col {column}    fn {function}")
            }
            (_, _, Some(function)) if !model.has_source => {
                format!("Stopped in {function} (no source)")
            }
            _ => "Stopped".into(),
        },
    }
}
