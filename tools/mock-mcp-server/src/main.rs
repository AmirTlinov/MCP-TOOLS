use std::{env, net::SocketAddr};

use anyhow::Result;
use axum::Router;
use rmcp::schemars::JsonSchema;
use rmcp::{
    ServiceExt,
    transport::{
        sse_server::SseServer,
        stdio,
        streamable_http_server::{
            session::local::LocalSessionManager, tower::StreamableHttpService,
        },
    },
};
use tokio::{net::TcpListener, signal};
use tokio_util::sync::CancellationToken;
use tracing_subscriber::EnvFilter;

#[derive(Clone, Default)]
struct MockServer;

impl MockServer {
    fn list_tools(&self) -> Vec<rmcp::model::Tool> {
        use rmcp::handler::server::wrapper::Parameters;
        fn schema_for<T: rmcp::schemars::JsonSchema + 'static>()
        -> std::sync::Arc<rmcp::model::JsonObject> {
            rmcp::handler::server::common::cached_schema_for_type::<T>()
        }
        vec![
            rmcp::model::Tool::new(
                "help",
                "Return a list of mock tools and usage hints.",
                schema_for::<Parameters<MockHelpArgs>>(),
            ),
            rmcp::model::Tool::new(
                "echo",
                "Echo back the supplied text payload.",
                schema_for::<Parameters<MockEchoArgs>>(),
            ),
            rmcp::model::Tool::new(
                "add",
                "Sum a list of numbers and return the total.",
                schema_for::<Parameters<MockAddArgs>>(),
            ),
        ]
    }

    fn call_tool(&self, request: rmcp::model::CallToolRequestParam) -> rmcp::model::CallToolResult {
        match request.name.as_ref() {
            "help" => {
                let description = serde_json::json!({
                    "tools": [
                        {"name": "help", "usage": "help"},
                        {"name": "echo", "usage": "echo text=\"hello\""},
                        {"name": "add", "usage": "add values=[1,2,3]"}
                    ]
                });
                rmcp::model::CallToolResult::structured(description)
            }
            "echo" => {
                let args = request
                    .arguments
                    .and_then(|map| {
                        serde_json::from_value::<MockEchoArgs>(serde_json::Value::Object(map)).ok()
                    })
                    .unwrap_or_default();
                rmcp::model::CallToolResult::structured(serde_json::json!({
                    "echoed": args.text,
                }))
            }
            "add" => {
                let args = request
                    .arguments
                    .and_then(|map| {
                        serde_json::from_value::<MockAddArgs>(serde_json::Value::Object(map)).ok()
                    })
                    .unwrap_or_default();
                let sum: f64 = args.values.iter().sum();
                rmcp::model::CallToolResult::structured(serde_json::json!({
                    "sum": sum,
                    "count": args.values.len(),
                }))
            }
            other => rmcp::model::CallToolResult::structured_error(serde_json::json!({
                "error": format!("unknown tool: {other}"),
            })),
        }
    }
}

#[derive(Debug, Clone, Default, serde::Deserialize, JsonSchema)]
struct MockHelpArgs {}

#[derive(Debug, Clone, Default, serde::Deserialize, JsonSchema)]
struct MockEchoArgs {
    #[serde(default)]
    text: String,
}

#[derive(Debug, Clone, Default, serde::Deserialize, JsonSchema)]
struct MockAddArgs {
    #[serde(default)]
    values: Vec<f64>,
}

impl rmcp::ServerHandler for MockServer {
    fn initialize(
        &self,
        request: rmcp::model::InitializeRequestParam,
        _context: rmcp::service::RequestContext<rmcp::service::RoleServer>,
    ) -> impl std::future::Future<Output = Result<rmcp::model::InitializeResult, rmcp::ErrorData>>
    + Send
    + '_ {
        async move {
            let capabilities = rmcp::model::ServerCapabilities::builder()
                .enable_tools()
                .enable_tool_list_changed()
                .build();
            let info = rmcp::model::ServerInfo {
                capabilities,
                server_info: rmcp::model::Implementation {
                    name: "mock-mcp-server".into(),
                    title: Some("Mock MCP Server".into()),
                    version: env!("CARGO_PKG_VERSION").into(),
                    icons: None,
                    website_url: None,
                },
                protocol_version: request.protocol_version,
                instructions: None,
            };
            tracing::info!("initialize complete");
            Ok(info)
        }
    }

    fn list_tools(
        &self,
        _request: Option<rmcp::model::PaginatedRequestParam>,
        _context: rmcp::service::RequestContext<rmcp::service::RoleServer>,
    ) -> impl std::future::Future<Output = Result<rmcp::model::ListToolsResult, rmcp::ErrorData>>
    + Send
    + '_ {
        let tools = self.list_tools();
        async move {
            Ok(rmcp::model::ListToolsResult {
                tools,
                next_cursor: None,
            })
        }
    }

    fn call_tool(
        &self,
        request: rmcp::model::CallToolRequestParam,
        _context: rmcp::service::RequestContext<rmcp::service::RoleServer>,
    ) -> impl std::future::Future<Output = Result<rmcp::model::CallToolResult, rmcp::ErrorData>>
    + Send
    + '_ {
        let response = self.call_tool(request);
        async move { Ok(response) }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let _ = tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .with_writer(std::io::stderr)
        .try_init();

    let sse_addr: SocketAddr = env::var("MOCK_SSE_ADDR")
        .unwrap_or_else(|_| "127.0.0.1:9100".into())
        .parse()?;
    let http_addr: SocketAddr = env::var("MOCK_HTTP_ADDR")
        .unwrap_or_else(|_| "127.0.0.1:9101".into())
        .parse()?;

    tracing::info!(%sse_addr, %http_addr, "mock server starting");

    let sse_handle = SseServer::serve(sse_addr)
        .await?
        .with_service(|| MockServer::default());

    let http_service: StreamableHttpService<MockServer, LocalSessionManager> =
        StreamableHttpService::new(
            || Ok(MockServer::default()),
            std::sync::Arc::new(LocalSessionManager::default()),
            Default::default(),
        );

    let http_router = Router::new().nest_service("/mcp", http_service);
    let http_listener = TcpListener::bind(http_addr).await?;
    let http_ct = CancellationToken::new();
    let http_task = tokio::spawn({
        let ct = http_ct.clone();
        async move {
            tracing::info!("http server listening");
            let _ = axum::serve(http_listener, http_router)
                .with_graceful_shutdown(async move { ct.cancelled().await })
                .await;
        }
    });

    let enable_stdio = env::var("MOCK_ENABLE_STDIO")
        .map(|v| !matches!(v.to_ascii_lowercase().as_str(), "0" | "false"))
        .unwrap_or(true);

    if enable_stdio {
        let server = MockServer::default().serve(stdio()).await?;
        tracing::info!("stdio server ready");
        server.waiting().await?;
    } else {
        tracing::info!("stdio disabled; waiting for shutdown signal");
        let _ = signal::ctrl_c().await;
    }

    tracing::info!("shutting down auxiliary transports");
    sse_handle.cancel();
    http_ct.cancel();
    let _ = http_task.await;
    Ok(())
}
