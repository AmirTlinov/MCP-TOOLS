use anyhow::Result;
use rmcp::{ErrorData as McpError, ServerHandler, model::*};

use crate::{
    app::{inspector_service::InspectorService, registry::ToolRegistry},
    domain::run::InspectionRun,
    shared::types::{CallRequest, ProbeRequest},
};

#[derive(Clone)]
pub struct InspectorServer {
    svc: InspectorService,
    registry: ToolRegistry,
}

impl InspectorServer {
    pub fn new() -> Self {
        Self {
            svc: InspectorService::new(),
            registry: ToolRegistry::default(),
        }
    }
}

impl ServerHandler for InspectorServer {
    fn initialize(
        &self,
        request: InitializeRequestParam,
        _context: rmcp::service::RequestContext<rmcp::service::RoleServer>,
    ) -> impl std::future::Future<Output = Result<InitializeResult, McpError>> + Send + '_ {
        use rmcp::model::{Implementation, ServerCapabilities, ServerInfo};
        tracing::info!(?request.client_info, %request.protocol_version, "initialize received");
        let capabilities = ServerCapabilities::builder()
            .enable_tools()
            .enable_tool_list_changed()
            .build();
        let init = ServerInfo {
            // echo back the protocol requested by client for compatibility
            protocol_version: request.protocol_version,
            capabilities,
            server_info: Implementation {
                name: "mcp-inspector".into(),
                title: Some("MCP Inspector".into()),
                version: env!("CARGO_PKG_VERSION").into(),
                icons: None,
                website_url: None,
            },
            instructions: None,
        };
        async move {
            tracing::info!("initialize ok");
            Ok(init)
        }
    }
    fn list_tools(
        &self,
        _request: Option<rmcp::model::PaginatedRequestParam>,
        _context: rmcp::service::RequestContext<rmcp::service::RoleServer>,
    ) -> impl std::future::Future<Output = Result<rmcp::model::ListToolsResult, McpError>> + Send + '_
    {
        let tools = self.registry.list();
        tracing::info!(count = tools.len(), "list_tools called");
        async move {
            Ok(ListToolsResult {
                tools,
                next_cursor: None,
            })
        }
    }

    fn call_tool(
        &self,
        request: CallToolRequestParam,
        _context: rmcp::service::RequestContext<rmcp::service::RoleServer>,
    ) -> impl std::future::Future<Output = Result<CallToolResult, McpError>> + Send + '_ {
        let this = self.clone();
        async move {
            tracing::info!(tool = %request.name, "call_tool received");
            let mut run = InspectionRun::new();
            run.start();
            let run_id = run.id;
            let name = request.name.as_ref();
            let args_map = request.arguments.unwrap_or_default();
            let args_val = serde_json::Value::Object(args_map);
            let mut failure = |msg: &str| {
                run.fail();
                CallToolResult::structured_error(serde_json::json!({"error": msg}))
            };

            let result: Result<CallToolResult, CallToolResult> = match name {
                "help" | "inspector_help" => {
                    let spec = serde_json::json!({
                      "server": {
                        "name": "mcp-inspector",
                        "version": env!("CARGO_PKG_VERSION"),
                        "protocol": "MCP",
                        "transport": "stdio"
                      },
                      "tldr": [
                        "1) help -> review examples",
                        "2) inspector_probe (stdio|sse|http)",
                        "3) inspector_list_tools (stdio only)",
                        "4) inspector_call (stdio only, transport lowercase)"
                      ],
                      "quick_start": [
                        {"tool":"inspector_probe","arguments":{"transport":"stdio","command":"uvx","args":["mcp-server-git"]},"expect":{"ok":true}},
                        {"tool":"inspector_list_tools","arguments":{"command":"uvx","args":["mcp-server-git"]},"expect":{"tools_min":1}},
                        {"tool":"inspector_call","env":{"INSPECTOR_STDIO_CMD":"uvx mcp-server-git"},"arguments":{"tool_name":"git_status","arguments_json":{"repo_path":"."}},"expect":{"structured_or_text":true}}
                      ],
                      "constraints": {
                        "inspector_probe": {"transports":["Stdio","Sse","Http"]},
                        "inspector_list_tools": {"transports":["Stdio"]},
                        "inspector_call": {"transports":["Stdio"]}
                      },
                      "env": {
                        "INSPECTOR_STDIO_CMD": {
                          "required_for": ["inspector_call"],
                          "format": "<command> [args...]",
                          "examples": {
                            "linux_macos_bash": "export INSPECTOR_STDIO_CMD='uvx mcp-server-git'",
                            "windows_powershell": "$Env:INSPECTOR_STDIO_CMD='uvx mcp-server-git'"
                          }
                        },
                        "RUST_LOG": {"default": "info"}
                      },
                      "tools": {
                        "help": {
                          "purpose": "Return this reference manual.",
                          "params_table": [],
                          "returns": {"type": "object", "description": "Structured reference payload"}
                        },
                        "inspector_probe": {
                          "purpose": "Check connectivity to a target MCP and capture version/latency.",
                          "params_table": [
                            {"name":"transport","type":"enum(stdio|sse|http)","required":false,"default":"stdio","desc":"Target transport (lowercase values)"},
                            {"name":"command","type":"string","required":false,"default":null,"desc":"Executable for the stdio server"},
                            {"name":"args","type":"array<string>","required":false,"default":[],"desc":"Process arguments"},
                            {"name":"env","type":"map<string,string>","required":false,"default":null,"desc":"Environment variables for the process"},
                            {"name":"cwd","type":"string","required":false,"default":null,"desc":"Working directory"},
                            {"name":"url","type":"string","required":false,"default":null,"desc":"SSE/HTTP endpoint"},
                            {"name":"headers","type":"map<string,string>","required":false,"default":null,"desc":"Headers for SSE/HTTP transports"},
                            {"name":"auth_token","type":"string","required":false,"default":null,"desc":"Bearer token for HTTP"},
                            {"name":"handshake_timeout_ms","type":"integer","required":false,"default":15000,"desc":"Handshake timeout in milliseconds"}
                          ],
                          "returns": {"ok":"bool","transport":"string","server_name":"string|null","version":"string|null","latency_ms":"integer|null","error":"string|null"}
                        },
                        "inspector_list_tools": {
                          "purpose": "List tools exposed by the target stdio MCP.",
                          "params_table": [
                            {"name":"command","type":"string","required":true,"default":null,"desc":"Target stdio server process"},
                            {"name":"args","type":"array<string>","required":false,"default":[],"desc":"Process arguments"},
                            {"name":"env","type":"map<string,string>","required":false,"default":null,"desc":"Environment variables"},
                            {"name":"cwd","type":"string","required":false,"default":null,"desc":"Working directory"}
                          ],
                          "returns": {"tools":"array<Tool>"}
                        },
                        "inspector_call": {
                          "purpose": "Invoke a tool on the target stdio MCP.",
                          "params_table": [
                            {"name":"tool_name","type":"string","required":true,"default":null,"desc":"Tool name on the target server"},
                            {"name":"arguments_json","type":"object","required":true,"default":{},"desc":"Tool arguments"},
                            {"name":"stdio","type":"object","required":false,"default":null,"desc":"Override stdio target: {command,args,env?,cwd?}"}
                          ],
                          "preconditions": ["Pass 'stdio' override or configure INSPECTOR_STDIO_CMD env"],
                          "returns": {"content":"array<Content>","structured_content":"object|null"}
                        }
                      },
                      "notes": {
                        "http_auth": "HTTP transport accepts Bearer tokens via ProbeRequest.auth_token.",
                        "sse_auth": "rmcp 0.8.1 lacks public support for SSE tokens; use HTTP transport if auth is required."
                      },
                      "errors": [
                        {"code":"MISSING_COMMAND","tool":"inspector_list_tools","reason":"command was not provided","action":"Pass command (and args if needed)"},
                        {"code":"MISSING_STDIO_CMD","tool":"inspector_call","reason":"INSPECTOR_STDIO_CMD not set","action":"Export the environment variable or provide stdio override"},
                        {"code":"UNKNOWN_TOOL","tool":"*","reason":"Requested tool is not registered","action":"Use help or inspector_list_tools"}
                      ]
                    });
                    Ok(CallToolResult::structured(spec))
                }
                // New names without dots (Codex-safe)
                "inspector_probe" | "inspector.probe" => {
                    match serde_json::from_value::<ProbeRequest>(args_val) {
                        Ok(req) => match this.svc.probe(req).await {
                            Ok(res) => Ok(CallToolResult::structured(
                                serde_json::to_value(res).unwrap(),
                            )),
                            Err(e) => Err(failure(&e.to_string())),
                        },
                        Err(e) => Err(failure(&e.to_string())),
                    }
                }
                "inspector_list_tools" | "inspector.list_tools" => {
                    match serde_json::from_value::<crate::shared::types::ProbeRequest>(args_val) {
                        Ok(req) => {
                            let program_args: Result<(String, Vec<String>), CallToolResult> =
                                if let (Some(cmd), Some(args)) =
                                    (req.command.clone(), req.args.clone())
                                {
                                    Ok((cmd, args))
                                } else if let Some(cmd) = req.command.clone() {
                                    crate::shared::utils::parse_command(&cmd)
                                        .map_err(|e| failure(&e.to_string()))
                                } else {
                                    Err(failure("command is required"))
                                };

                            match program_args {
                                Ok((program, args)) => {
                                    match this
                                        .svc
                                        .list_tools_stdio(
                                            program,
                                            args,
                                            req.env.clone(),
                                            req.cwd.clone(),
                                        )
                                        .await
                                    {
                                        Ok(tools) => Ok(CallToolResult::structured(
                                            serde_json::json!({"tools": tools}),
                                        )),
                                        Err(e) => Err(failure(&e.to_string())),
                                    }
                                }
                                Err(err) => Err(err),
                            }
                        }
                        Err(e) => Err(failure(&e.to_string())),
                    }
                }
                "inspector_call" | "inspector.call" => {
                    match serde_json::from_value::<CallRequest>(args_val) {
                        Ok(req) => {
                            // priority: explicit stdio target first, then environment fallback
                            if let Some(target) = req.stdio.as_ref() {
                                match this
                                    .svc
                                    .call_stdio(
                                        target.command.clone(),
                                        target.args.clone(),
                                        target.env.clone(),
                                        target.cwd.clone(),
                                        req.tool_name,
                                        req.arguments_json,
                                    )
                                    .await
                                {
                                    Ok(res) => Ok(res),
                                    Err(e) => Err(failure(&e.to_string())),
                                }
                            } else {
                                let default_cmd = std::env::var("INSPECTOR_STDIO_CMD").ok();
                                let fallback: Result<(String, Vec<String>), CallToolResult> =
                                    if let Some(cmd) = default_cmd {
                                        crate::shared::utils::parse_command(&cmd)
                                            .map_err(|e| failure(&e.to_string()))
                                    } else {
                                        Err(failure(
                                            "INSPECTOR_STDIO_CMD env is required or pass 'stdio' target",
                                        ))
                                    };

                                match fallback {
                                    Ok((program, args)) => {
                                        match this
                                            .svc
                                            .call_stdio(
                                                program,
                                                args,
                                                None,
                                                None,
                                                req.tool_name,
                                                req.arguments_json,
                                            )
                                            .await
                                        {
                                            Ok(res) => Ok(res),
                                            Err(e) => Err(failure(&e.to_string())),
                                        }
                                    }
                                    Err(err) => Err(err),
                                }
                            }
                        }
                        Err(e) => Err(failure(&e.to_string())),
                    }
                }
                _ => Err(failure("unknown tool")),
            };

            match result {
                Ok(success) => {
                    run.capture();
                    tracing::info!(%run_id, tool = %request.name, "call_tool success");
                    Ok(success)
                }
                Err(err) => {
                    tracing::warn!(%run_id, tool = %request.name, "call_tool returned business error");
                    Ok(err)
                }
            }
        }
    }

    fn get_info(&self) -> rmcp::model::ServerInfo {
        use rmcp::model::{Implementation, ServerCapabilities, ServerInfo};
        let capabilities = ServerCapabilities::builder()
            .enable_tools()
            .enable_tool_list_changed()
            .build();
        ServerInfo {
            capabilities,
            server_info: Implementation {
                name: "mcp-inspector".into(),
                title: Some("MCP Inspector".into()),
                version: env!("CARGO_PKG_VERSION").into(),
                icons: None,
                website_url: None,
            },
            ..Default::default()
        }
    }

    fn on_initialized(
        &self,
        context: rmcp::service::NotificationContext<rmcp::service::RoleServer>,
    ) -> impl std::future::Future<Output = ()> + Send + '_ {
        async move {
            tracing::info!("on_initialized -> schedule list_changed");
            let peer = context.peer.clone();
            tokio::spawn(async move {
                if let Err(e) = peer.notify_tool_list_changed().await {
                    tracing::warn!(error=%e, "tools/list_changed notify failed");
                } else {
                    tracing::info!("tools/list_changed notified");
                }
            });
        }
    }
}
