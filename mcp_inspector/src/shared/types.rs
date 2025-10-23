use rmcp::schemars::JsonSchema;
use serde::{Deserialize, Serialize};

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
    // optional stdio target overrides (если задан — ENV игнорируется)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stdio: Option<StdioTarget>,
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
