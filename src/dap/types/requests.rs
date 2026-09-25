use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use super::Request;
use super::objects::{
    Breakpoint, Capabilities, Scope, Source, SourceBreakpoint, StackFrame, SteppingGranularity,
    Thread, Variable,
};

pub enum Initialize {}

impl Request for Initialize {
    const COMMAND: &'static str = "initialize";
    type Arguments = InitializeArguments;
    type Response = Capabilities;
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InitializeArguments {
    #[serde(rename = "adapterID")]
    pub adapter_id: String,
    #[serde(rename = "clientID", default, skip_serializing_if = "Option::is_none")]
    pub client_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub client_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub locale: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lines_start_at1: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub columns_start_at1: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path_format: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub supports_variable_type: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub supports_variable_paging: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub supports_run_in_terminal_request: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub supports_memory_references: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub supports_progress_reporting: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub supports_invalidated_event: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub supports_memory_event: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub supports_args_can_be_interpreted_by_shell: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub supports_start_debugging_request: Option<bool>,
    #[serde(
        rename = "supportsANSIStyling",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub supports_ansi_styling: Option<bool>,
}

impl InitializeArguments {
    /// Client capabilities we currently advertise. Reverse-request flags stay off.
    pub fn new(adapter_id: impl Into<String>) -> Self {
        Self {
            adapter_id: adapter_id.into(),
            client_id: Some("argus-tui".into()),
            client_name: Some("argus-tui".into()),
            locale: None,
            lines_start_at1: Some(true),
            columns_start_at1: Some(true),
            path_format: Some("path".into()),
            supports_variable_type: Some(true),
            supports_variable_paging: None,
            supports_run_in_terminal_request: None,
            supports_memory_references: None,
            supports_progress_reporting: None,
            supports_invalidated_event: None,
            supports_memory_event: None,
            supports_args_can_be_interpreted_by_shell: None,
            supports_start_debugging_request: None,
            supports_ansi_styling: None,
        }
    }
}

pub enum Launch {}

impl Request for Launch {
    const COMMAND: &'static str = "launch";
    type Arguments = LaunchArguments;
    type Response = ();
}

/// `launch` arguments: DAP's `noDebug` plus the `lldb-dap` fields we use.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LaunchArguments {
    pub program: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub args: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cwd: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub env: Option<BTreeMap<String, String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stop_on_entry: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub no_debug: Option<bool>,
    #[serde(flatten, default)]
    pub extra: Map<String, Value>,
}

impl LaunchArguments {
    pub fn new(program: impl Into<String>) -> Self {
        Self {
            program: program.into(),
            args: None,
            cwd: None,
            env: None,
            stop_on_entry: None,
            no_debug: None,
            extra: Map::new(),
        }
    }
}

pub enum ConfigurationDone {}

impl Request for ConfigurationDone {
    const COMMAND: &'static str = "configurationDone";
    type Arguments = ();
    type Response = ();
}

pub enum SetBreakpoints {}

impl Request for SetBreakpoints {
    const COMMAND: &'static str = "setBreakpoints";
    type Arguments = SetBreakpointsArguments;
    type Response = SetBreakpointsResponse;
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SetBreakpointsArguments {
    pub source: Source,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub breakpoints: Option<Vec<SourceBreakpoint>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_modified: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct SetBreakpointsResponse {
    #[serde(default)]
    pub breakpoints: Vec<Breakpoint>,
}

pub enum Threads {}

impl Request for Threads {
    const COMMAND: &'static str = "threads";
    type Arguments = ();
    type Response = ThreadsResponse;
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ThreadsResponse {
    #[serde(default)]
    pub threads: Vec<Thread>,
}

pub enum StackTrace {}

impl Request for StackTrace {
    const COMMAND: &'static str = "stackTrace";
    type Arguments = StackTraceArguments;
    type Response = StackTraceResponse;
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StackTraceArguments {
    pub thread_id: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub start_frame: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub levels: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct StackTraceResponse {
    #[serde(default)]
    pub stack_frames: Vec<StackFrame>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub total_frames: Option<i64>,
}

pub enum Scopes {}

impl Request for Scopes {
    const COMMAND: &'static str = "scopes";
    type Arguments = ScopesArguments;
    type Response = ScopesResponse;
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScopesArguments {
    pub frame_id: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ScopesResponse {
    #[serde(default)]
    pub scopes: Vec<Scope>,
}

pub enum Variables {}

impl Request for Variables {
    const COMMAND: &'static str = "variables";
    type Arguments = VariablesArguments;
    type Response = VariablesResponse;
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VariablesArguments {
    pub variables_reference: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub filter: Option<VariablesFilter>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub start: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub count: Option<i64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum VariablesFilter {
    Indexed,
    Named,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct VariablesResponse {
    #[serde(default)]
    pub variables: Vec<Variable>,
}

pub enum Continue {}

impl Request for Continue {
    const COMMAND: &'static str = "continue";
    type Arguments = ContinueArguments;
    type Response = ContinueResponse;
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContinueArguments {
    pub thread_id: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub single_thread: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ContinueResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub all_threads_continued: Option<bool>,
}

pub enum Next {}

impl Request for Next {
    const COMMAND: &'static str = "next";
    type Arguments = NextArguments;
    type Response = ();
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NextArguments {
    pub thread_id: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub single_thread: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub granularity: Option<SteppingGranularity>,
}

pub enum StepIn {}

impl Request for StepIn {
    const COMMAND: &'static str = "stepIn";
    type Arguments = StepInArguments;
    type Response = ();
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StepInArguments {
    pub thread_id: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub single_thread: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_id: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub granularity: Option<SteppingGranularity>,
}

pub enum StepOut {}

impl Request for StepOut {
    const COMMAND: &'static str = "stepOut";
    type Arguments = StepOutArguments;
    type Response = ();
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StepOutArguments {
    pub thread_id: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub single_thread: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub granularity: Option<SteppingGranularity>,
}

pub enum Evaluate {}

impl Request for Evaluate {
    const COMMAND: &'static str = "evaluate";
    type Arguments = EvaluateArguments;
    type Response = EvaluateResponse;
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EvaluateArguments {
    pub expression: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub frame_id: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context: Option<EvaluateContext>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EvaluateContext {
    #[serde(rename = "watch")]
    Watch,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct EvaluateResponse {
    #[serde(default)]
    pub result: String,
    #[serde(rename = "type", default, skip_serializing_if = "Option::is_none")]
    pub type_field: Option<String>,
    #[serde(default)]
    pub variables_reference: i64,
}

pub enum Disconnect {}

impl Request for Disconnect {
    const COMMAND: &'static str = "disconnect";
    type Arguments = DisconnectArguments;
    type Response = ();
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct DisconnectArguments {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub restart: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub terminate_debuggee: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub suspend_debuggee: Option<bool>,
}
