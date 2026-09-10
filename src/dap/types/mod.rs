//! Hand-written Debug Adapter Protocol JSON types.
//!
//! Framing (`Content-Length`) lives in `dap::framing`. Process I/O lives in
//! `dap::client`. This module is only the JSON shapes.
//!
//! Re-exports are the client-facing surface; they look unused until `client.rs`
//! starts sending typed requests.
#![allow(dead_code, unused_imports)]

mod events;
mod objects;
mod requests;

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub use events::{
    BreakpointEvent, BreakpointEventReason, ContinuedEvent, Event, ExitedEvent, OutputCategory,
    OutputEvent, ProcessEvent, ProcessStartMethod, StoppedEvent, StoppedReason, TerminatedEvent,
    ThreadEvent, ThreadEventReason,
};
pub use objects::{
    Breakpoint, Capabilities, ErrorBody, ErrorMessage, Scope, Source, SourceBreakpoint, StackFrame,
    StackFramePresentationHint, SteppingGranularity, Thread, Variable,
};
pub use requests::{
    ConfigurationDone, Continue, ContinueArguments, ContinueResponse, Disconnect,
    DisconnectArguments, Initialize, InitializeArguments, Launch, LaunchArguments, Next,
    NextArguments, Scopes, ScopesArguments, ScopesResponse, SetBreakpoints,
    SetBreakpointsArguments, SetBreakpointsResponse, StackTrace, StackTraceArguments,
    StackTraceResponse, StepIn, StepInArguments, StepOut, StepOutArguments, Threads,
    ThreadsResponse, Variables, VariablesArguments, VariablesFilter, VariablesResponse,
};

/// Sequence number shared by requests, responses, and events.
pub type Seq = i64;

/// Client-initiated request. Marker types in [`requests`] implement this so a
/// future client can write `send::<StackTrace>(args)`.
pub trait Request {
    const COMMAND: &'static str;
    type Arguments: Serialize;
    type Response: DeserializeOwned + Default + 'static;
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum Message {
    #[serde(rename = "request")]
    Request(RequestMessage),
    #[serde(rename = "response")]
    Response(ResponseMessage),
    #[serde(rename = "event")]
    Event(EventMessage),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RequestMessage {
    pub seq: Seq,
    pub command: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub arguments: Option<Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ResponseMessage {
    pub seq: Seq,
    pub request_seq: Seq,
    pub success: bool,
    pub command: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub body: Option<Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EventMessage {
    pub seq: Seq,
    pub event: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub body: Option<Value>,
}

#[derive(Debug, thiserror::Error)]
pub enum ProtocolError {
    #[error(
        "{command} failed{}",
        .message.as_ref().map(|m| format!(": {m}")).unwrap_or_default()
    )]
    Failed {
        command: String,
        message: Option<String>,
        body: Option<Box<ErrorBody>>,
    },
    #[error("failed to decode {kind} {name}: {source}")]
    Decode {
        kind: &'static str,
        name: String,
        #[source]
        source: serde_json::Error,
    },
    #[error("unexpected {expected}")]
    Unexpected {
        expected: &'static str,
        got: Box<Message>,
    },
}

impl RequestMessage {
    pub fn new<R: Request>(seq: Seq, args: R::Arguments) -> Result<Self, serde_json::Error> {
        Ok(Self {
            seq,
            command: R::COMMAND.to_string(),
            arguments: omit_empty_args(serde_json::to_value(args)?),
        })
    }
}

impl ResponseMessage {
    pub fn decode_success<R: Request>(&self) -> Result<R::Response, ProtocolError> {
        if !self.success {
            return Err(ProtocolError::Failed {
                command: self.command.clone(),
                message: self.message.clone(),
                body: self.error_body().map(Box::new),
            });
        }
        decode_success_body::<R::Response>(self.body.as_ref(), R::COMMAND)
    }

    pub fn error_body(&self) -> Option<ErrorBody> {
        self.body
            .as_ref()
            .and_then(|body| serde_json::from_value(body.clone()).ok())
    }
}

impl EventMessage {
    pub fn parse(&self) -> Result<Event, ProtocolError> {
        events::parse_event(&self.event, self.body.as_ref())
    }
}

fn omit_empty_args(value: Value) -> Option<Value> {
    match value {
        Value::Null => None,
        Value::Object(map) if map.is_empty() => None,
        other => Some(other),
    }
}

fn decode_success_body<T: DeserializeOwned + Default + 'static>(
    body: Option<&Value>,
    command: &str,
) -> Result<T, ProtocolError> {
    match body {
        None | Some(Value::Null) => Ok(T::default()),
        Some(Value::Object(map))
            if map.is_empty() && std::any::TypeId::of::<T>() == std::any::TypeId::of::<()>() =>
        {
            Ok(T::default())
        }
        Some(value) => {
            serde_json::from_value(value.clone()).map_err(|source| ProtocolError::Decode {
                kind: "response",
                name: command.to_string(),
                source,
            })
        }
    }
}

#[cfg(test)]
mod tests;
