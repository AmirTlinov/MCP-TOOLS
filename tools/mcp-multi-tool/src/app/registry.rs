use rmcp::{handler::server::wrapper::Parameters, model::*};
use std::sync::Arc;

fn schema_for<T: rmcp::schemars::JsonSchema + 'static>() -> Arc<rmcp::model::JsonObject> {
    rmcp::handler::server::common::cached_schema_for_type::<T>()
}

#[derive(Default, Clone)]
pub struct ToolRegistry;

impl ToolRegistry {
    pub fn list(&self) -> Vec<Tool> {
        vec![
            Tool::new(
                "help",
                "Deterministic reference manual for every tool exposed by this server.",
                // Some clients expect input_schema to be an object rather than null
                schema_for::<Parameters<crate::shared::types::EmptyArgs>>(),
            ),
            Tool::new(
                "inspector_probe",
                "Connect to a target MCP and retrieve version/latency details.",
                schema_for::<Parameters<crate::shared::types::ProbeRequest>>(),
            ),
            Tool::new(
                "inspector_list_tools",
                "List target MCP tools across stdio/SSE/HTTP transports.",
                schema_for::<Parameters<crate::shared::types::ProbeRequest>>(),
            ),
            Tool::new(
                "inspector_describe",
                "Describe a target MCP tool including schemas and annotations.",
                schema_for::<Parameters<crate::shared::types::DescribeRequest>>(),
            ),
            Tool::new(
                "inspector_call",
                "Call a target MCP tool via stdio/SSE/HTTP transports.",
                schema_for::<Parameters<crate::shared::types::CallRequest>>(),
            ),
        ]
    }
}
