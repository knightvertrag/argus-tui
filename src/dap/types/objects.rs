use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// DAP structured error message (`Message` in the spec).
///
/// Named `ErrorMessage` so it does not collide with the wire [`super::Message`] enum.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ErrorMessage {
    pub id: i64,
    pub format: String,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub variables: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub send_telemetry: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub show_user: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url_label: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ErrorBody {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<ErrorMessage>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Source {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_reference: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StackFrame {
    pub id: i64,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<Source>,
    pub line: i64,
    pub column: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub end_line: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub end_column: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub presentation_hint: Option<StackFramePresentationHint>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum StackFramePresentationHint {
    Normal,
    Label,
    Subtle,
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Scope {
    pub name: String,
    pub variables_reference: i64,
    pub expensive: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub named_variables: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub indexed_variables: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Variable {
    pub name: String,
    pub value: String,
    #[serde(rename = "type", default, skip_serializing_if = "Option::is_none")]
    pub type_field: Option<String>,
    pub variables_reference: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub evaluate_name: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Thread {
    pub id: i64,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Breakpoint {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<i64>,
    pub verified: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<Source>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub line: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub column: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceBreakpoint {
    pub line: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub column: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub condition: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hit_condition: Option<String>,
}

/// Adapter capabilities returned by `initialize`.
///
/// Named flags are the ones the session will branch on. Everything else is
/// preserved in [`Self::extra`] so an `lldb-dap` response is never lossy.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Capabilities {
    #[serde(default)]
    pub supports_configuration_done_request: bool,
    #[serde(default)]
    pub supports_conditional_breakpoints: bool,
    #[serde(default)]
    pub supports_set_variable: bool,
    #[serde(default)]
    pub supports_terminate_request: bool,
    #[serde(default)]
    pub support_terminate_debuggee: bool,
    #[serde(default)]
    pub supports_single_thread_execution_requests: bool,
    #[serde(default)]
    pub supports_delayed_stack_trace_loading: bool,
    #[serde(default)]
    pub supports_cancel_request: bool,
    #[serde(flatten, default)]
    pub extra: serde_json::Map<String, Value>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SteppingGranularity {
    Statement,
    Line,
    Instruction,
    #[serde(other)]
    Unknown,
}
