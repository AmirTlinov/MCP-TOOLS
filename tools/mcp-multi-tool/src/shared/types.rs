use rmcp::schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Default)]
pub struct EmptyArgs {}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum TargetTransportKind {
    Stdio,
    Sse,
    Http,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Default)]
pub struct ProbeRequest {
    pub transport: Option<TargetTransportKind>,
    // stdio
    pub command: Option<String>,
    pub args: Option<Vec<String>>,
    pub env: Option<std::collections::BTreeMap<String, String>>,
    pub cwd: Option<String>,
    // network
    pub url: Option<String>,
    pub headers: Option<std::collections::BTreeMap<String, String>>,
    pub auth_token: Option<String>,
    // behavior
    pub handshake_timeout_ms: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ProbeResult {
    pub ok: bool,
    pub transport: String,
    pub server_name: Option<String>,
    pub version: Option<String>,
    pub latency_ms: Option<u64>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CallRequest {
    pub tool_name: String,
    pub arguments_json: serde_json::Value,
    pub idempotency_key: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub external_reference: Option<String>,
    // optional stdio target overrides (takes precedence over environment defaults)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stdio: Option<StdioTarget>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sse: Option<SseTarget>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub http: Option<HttpTarget>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct DescribeRequest {
    pub tool_name: String,
    #[serde(flatten)]
    #[serde(default)]
    pub probe: ProbeRequest,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Default)]
pub struct StdioTarget {
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub env: Option<std::collections::BTreeMap<String, String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cwd: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Default)]
pub struct TargetDescriptor {
    pub transport: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub headers: Option<std::collections::BTreeMap<String, String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Default)]
pub struct SseTarget {
    pub url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub headers: Option<std::collections::BTreeMap<String, String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub handshake_timeout_ms: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Default)]
pub struct HttpTarget {
    pub url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub headers: Option<std::collections::BTreeMap<String, String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auth_token: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub handshake_timeout_ms: Option<u64>,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct InspectionRunEvent {
    #[schemars(with = "String")]
    pub event_id: uuid::Uuid,
    #[schemars(with = "String")]
    pub run_id: uuid::Uuid,
    pub tool_name: String,
    pub state: String,
    pub started_at: String,
    pub duration_ms: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target: Option<TargetDescriptor>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub request: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub response: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub idempotency_key: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub external_reference: Option<String>,
}
