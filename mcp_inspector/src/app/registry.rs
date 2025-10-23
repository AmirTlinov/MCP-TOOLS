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
                "Детерминированная формальная справка по использованию инструментов этого сервера.",
                // Некоторые клиенты ожидают input_schema типа object, а не null
                schema_for::<Parameters<crate::shared::types::EmptyArgs>>(),
            ),
            Tool::new(
                "inspector_probe",
                "Подключиться к целевому MCP и получить версию/latency",
                schema_for::<Parameters<crate::shared::types::ProbeRequest>>(),
            ),
            Tool::new(
                "inspector_list_tools",
                "Получить список инструментов целевого MCP через stdio",
                schema_for::<Parameters<crate::shared::types::ProbeRequest>>(),
            ),
            Tool::new(
                "inspector_call",
                "Вызвать инструмент целевого MCP через stdio",
                schema_for::<Parameters<crate::shared::types::CallRequest>>(),
            ),
        ]
    }
}
