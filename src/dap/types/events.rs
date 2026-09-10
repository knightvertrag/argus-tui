use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::ProtocolError;
use super::objects::Breakpoint;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Event {
    Initialized,
    Stopped(StoppedEvent),
    Continued(ContinuedEvent),
    Exited(ExitedEvent),
    Terminated(TerminatedEvent),
    Thread(ThreadEvent),
    Output(OutputEvent),
    Process(ProcessEvent),
    Breakpoint(BreakpointEvent),
    Unknown { event: String, body: Option<Value> },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StoppedEvent {
    pub reason: StoppedReason,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thread_id: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preserve_focus_hint: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub all_threads_stopped: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hit_breakpoint_ids: Option<Vec<i64>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum StoppedReason {
    #[serde(rename = "step")]
    Step,
    #[serde(rename = "breakpoint")]
    Breakpoint,
    #[serde(rename = "exception")]
    Exception,
    #[serde(rename = "pause")]
    Pause,
    #[serde(rename = "entry")]
    Entry,
    #[serde(rename = "goto")]
    Goto,
    #[serde(rename = "function breakpoint")]
    FunctionBreakpoint,
    #[serde(rename = "data breakpoint")]
    DataBreakpoint,
    #[serde(rename = "instruction breakpoint")]
    InstructionBreakpoint,
    #[serde(untagged)]
    Other(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContinuedEvent {
    pub thread_id: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub all_threads_continued: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExitedEvent {
    pub exit_code: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct TerminatedEvent {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub restart: Option<Value>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreadEvent {
    pub reason: ThreadEventReason,
    pub thread_id: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ThreadEventReason {
    #[serde(rename = "started")]
    Started,
    #[serde(rename = "exited")]
    Exited,
    #[serde(untagged)]
    Other(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OutputEvent {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub category: Option<OutputCategory>,
    pub output: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OutputCategory {
    #[serde(rename = "console")]
    Console,
    #[serde(rename = "important")]
    Important,
    #[serde(rename = "stdout")]
    Stdout,
    #[serde(rename = "stderr")]
    Stderr,
    #[serde(rename = "telemetry")]
    Telemetry,
    #[serde(untagged)]
    Other(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProcessEvent {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub system_process_id: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub is_local_process: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub start_method: Option<ProcessStartMethod>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pointer_size: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProcessStartMethod {
    #[serde(rename = "launch")]
    Launch,
    #[serde(rename = "attach")]
    Attach,
    #[serde(rename = "attachForSuspendedLaunch")]
    AttachForSuspendedLaunch,
    #[serde(untagged)]
    Other(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BreakpointEvent {
    pub reason: BreakpointEventReason,
    pub breakpoint: Breakpoint,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum BreakpointEventReason {
    #[serde(rename = "changed")]
    Changed,
    #[serde(rename = "new")]
    New,
    #[serde(rename = "removed")]
    Removed,
    #[serde(untagged)]
    Other(String),
}

pub(super) fn parse_event(event: &str, body: Option<&Value>) -> Result<Event, ProtocolError> {
    match event {
        "initialized" => Ok(Event::Initialized),
        "stopped" => Ok(Event::Stopped(decode_required(body, event)?)),
        "continued" => Ok(Event::Continued(decode_required(body, event)?)),
        "exited" => Ok(Event::Exited(decode_required(body, event)?)),
        "terminated" => Ok(Event::Terminated(decode_optional(body, event)?)),
        "thread" => Ok(Event::Thread(decode_required(body, event)?)),
        "output" => Ok(Event::Output(decode_required(body, event)?)),
        "process" => Ok(Event::Process(decode_required(body, event)?)),
        "breakpoint" => Ok(Event::Breakpoint(decode_required(body, event)?)),
        other => Ok(Event::Unknown {
            event: other.to_string(),
            body: body.cloned(),
        }),
    }
}

fn decode_required<T: serde::de::DeserializeOwned>(
    body: Option<&Value>,
    name: &str,
) -> Result<T, ProtocolError> {
    let value = body.cloned().unwrap_or(Value::Null);
    serde_json::from_value(value).map_err(|source| ProtocolError::Decode {
        kind: "event",
        name: name.to_string(),
        source,
    })
}

fn decode_optional<T: serde::de::DeserializeOwned + Default>(
    body: Option<&Value>,
    name: &str,
) -> Result<T, ProtocolError> {
    match body {
        None | Some(Value::Null) => Ok(T::default()),
        Some(value) => {
            serde_json::from_value(value.clone()).map_err(|source| ProtocolError::Decode {
                kind: "event",
                name: name.to_string(),
                source,
            })
        }
    }
}
