use anyhow::Result;
use rmcp::{ErrorData as McpError, ServerHandler, model::*};
use serde_json::{Value, json};
use std::{sync::Arc, time::Instant};
use time::OffsetDateTime;

use crate::{
    app::{inspector_service::InspectorService, registry::ToolRegistry},
    domain::run::InspectionRun,
    infra::{config::IdempotencyConflictPolicy, outbox::Outbox},
    shared::{
        idempotency::{ClaimOutcome, IdempotencyStore},
        types::{CallRequest, InspectionRunEvent, ProbeRequest, TargetDescriptor},
    },
};

#[derive(Clone)]
pub struct InspectorServer {
    svc: InspectorService,
    registry: ToolRegistry,
    outbox: Arc<Outbox>,
    idempotency: Arc<IdempotencyStore>,
    conflict_policy: IdempotencyConflictPolicy,
}

impl InspectorServer {
    pub fn new(
        svc: InspectorService,
        registry: ToolRegistry,
        outbox: Arc<Outbox>,
        idempotency: Arc<IdempotencyStore>,
        conflict_policy: IdempotencyConflictPolicy,
    ) -> Self {
        Self {
            svc,
            registry,
            outbox,
            idempotency,
            conflict_policy,
        }
    }

    fn idempotency_conflict_response(
        &self,
        existing: Option<InspectionRunEvent>,
        message: &str,
    ) -> CallToolResult {
        let payload = match existing {
            Some(event) => json!({
                "error": message,
                "code": "IDEMPOTENCY_CONFLICT",
                "event": event,
            }),
            None => json!({
                "error": message,
                "code": "IDEMPOTENCY_CONFLICT",
            }),
        };
        CallToolResult::structured_error(payload)
    }

    fn return_existing_event(&self, event: InspectionRunEvent) -> CallToolResult {
        CallToolResult::structured(json!({
            "status": "duplicate",
            "event": event,
        }))
    }

    fn build_event(
        &self,
        run: &InspectionRun,
        request: &CallRequest,
        started_at: OffsetDateTime,
        duration_ms: u64,
        target: Option<TargetDescriptor>,
        response: Option<Value>,
        error: Option<String>,
    ) -> InspectionRunEvent {
        let started_at_str = started_at.to_string();
        InspectionRunEvent {
            event_id: uuid::Uuid::new_v4(),
            run_id: run.id,
            tool_name: request.tool_name.clone(),
            state: run.state.as_str().to_string(),
            started_at: started_at_str,
            duration_ms,
            target,
            request: serde_json::to_value(request).ok(),
            response,
            error,
        }
    }

    fn snapshot_result(&self, result: &CallToolResult) -> Option<Value> {
        serde_json::to_value(result).ok()
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
                name: "mcp-multi-tool".into(),
                title: Some("MCP MultiTool".into()),
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
                        "name": "mcp-multi-tool",
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
                        "inspector_list_tools": {"transports":["Stdio","Sse","Http"]},
                        "inspector_call": {"transports":["Stdio","Sse","Http"]}
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
                            {"name":"cwd","type":"string","required":false,"default":null,"desc":"Working directory"},
                            {"name":"sse","type":"object","required":false,"default":null,"desc":"SSE override: {url,headers?,handshake_timeout_ms?}"},
                            {"name":"http","type":"object","required":false,"default":null,"desc":"HTTP override: {url,headers?,auth_token?,handshake_timeout_ms?}"}
                          ],
                          "returns": {"tools":"array<Tool>"}
                        },
                        "inspector_call": {
                          "purpose": "Invoke a tool on the target stdio MCP.",
                          "params_table": [
                            {"name":"tool_name","type":"string","required":true,"default":null,"desc":"Tool name on the target server"},
                            {"name":"arguments_json","type":"object","required":true,"default":{},"desc":"Tool arguments"},
                            {"name":"stdio","type":"object","required":false,"default":null,"desc":"Override stdio target: {command,args,env?,cwd?}"},
                            {"name":"sse","type":"object","required":false,"default":null,"desc":"SSE override: {url,headers?,handshake_timeout_ms?}"},
                            {"name":"http","type":"object","required":false,"default":null,"desc":"HTTP override: {url,headers?,auth_token?,handshake_timeout_ms?}"}
                          ],
                          "preconditions": ["Provide one of stdio/sse/http overrides or configure INSPECTOR_STDIO_CMD env"],
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
                            let started_at = OffsetDateTime::now_utc();
                            let timer = Instant::now();
                            let mut target_descriptor = TargetDescriptor {
                                transport: "stdio".into(),
                                command: None,
                                url: None,
                                headers: None,
                            };
                            let mut claimed_key: Option<String> = None;
                            if let Some(key) = req.idempotency_key.clone() {
                                match this.idempotency.claim(&key) {
                                    ClaimOutcome::Accepted => {
                                        claimed_key = Some(key);
                                    }
                                    ClaimOutcome::InFlight => {
                                        run.fail();
                                        let err = this.idempotency_conflict_response(
                                            None,
                                            "idempotency key already in-flight",
                                        );
                                        return Ok(err);
                                    }
                                    ClaimOutcome::Completed(event) => {
                                        return match this.conflict_policy {
                                            IdempotencyConflictPolicy::ReturnExisting => {
                                                run.capture();
                                                Ok(this.return_existing_event(event))
                                            }
                                            IdempotencyConflictPolicy::Conflict409 => {
                                                run.fail();
                                                Ok(this.idempotency_conflict_response(
                                                    Some(event),
                                                    "idempotency conflict",
                                                ))
                                            }
                                        };
                                    }
                                }
                            }
                            let call_outcome = if let Some(http) = req.http.as_ref() {
                                target_descriptor.transport = "http".into();
                                target_descriptor.url = Some(http.url.clone());
                                target_descriptor.headers = http.headers.clone();
                                this.svc
                                    .call_http(
                                        http,
                                        req.tool_name.clone(),
                                        req.arguments_json.clone(),
                                    )
                                    .await
                            } else if let Some(sse) = req.sse.as_ref() {
                                target_descriptor.transport = "sse".into();
                                target_descriptor.url = Some(sse.url.clone());
                                target_descriptor.headers = sse.headers.clone();
                                this.svc
                                    .call_sse(
                                        sse,
                                        req.tool_name.clone(),
                                        req.arguments_json.clone(),
                                    )
                                    .await
                            } else if let Some(target) = req.stdio.as_ref() {
                                target_descriptor.transport = "stdio".into();
                                target_descriptor.command = Some(target.command.clone());
                                this.svc
                                    .call_stdio(
                                        target.command.clone(),
                                        target.args.clone(),
                                        target.env.clone(),
                                        target.cwd.clone(),
                                        req.tool_name.clone(),
                                        req.arguments_json.clone(),
                                    )
                                    .await
                            } else {
                                let default_cmd = std::env::var("INSPECTOR_STDIO_CMD").ok();
                                let fallback: Result<(String, Vec<String>), CallToolResult> =
                                    if let Some(cmd) = default_cmd {
                                        crate::shared::utils::parse_command(&cmd)
                                            .map(|(program, args)| {
                                                target_descriptor.transport = "stdio".into();
                                                target_descriptor.command = Some(program.clone());
                                                (program, args)
                                            })
                                            .map_err(|e| failure(&e.to_string()))
                                    } else {
                                        Err(failure(
                                            "INSPECTOR_STDIO_CMD env is required or pass 'stdio' target",
                                        ))
                                    };
                                match fallback {
                                    Ok((program, args)) => {
                                        this.svc
                                            .call_stdio(
                                                program.clone(),
                                                args,
                                                None,
                                                None,
                                                req.tool_name.clone(),
                                                req.arguments_json.clone(),
                                            )
                                            .await
                                    }
                                    Err(err) => return Ok(err),
                                }
                            };
                            match call_outcome {
                                Ok(res) => {
                                    run.capture();
                                    let duration_ms = timer.elapsed().as_millis() as u64;
                                    let event = this.build_event(
                                        &run,
                                        &req,
                                        started_at,
                                        duration_ms,
                                        Some(target_descriptor),
                                        this.snapshot_result(&res),
                                        None,
                                    );
                                    if let Err(e) = this.outbox.append(&event) {
                                        tracing::error!(%run_id, error=%e, "failed to append outbox event");
                                    }
                                    if let Some(key) = claimed_key {
                                        this.idempotency.complete(&key, event);
                                    }
                                    Ok(res)
                                }
                                Err(error) => {
                                    run.fail();
                                    let message = error.to_string();
                                    let duration_ms = timer.elapsed().as_millis() as u64;
                                    let event = this.build_event(
                                        &run,
                                        &req,
                                        started_at,
                                        duration_ms,
                                        Some(target_descriptor),
                                        None,
                                        Some(message.clone()),
                                    );
                                    if let Err(e) = this.outbox.append(&event) {
                                        tracing::error!(%run_id, error=%e, "failed to append failed event to outbox");
                                    }
                                    if let Some(key) = claimed_key {
                                        this.idempotency.complete(&key, event);
                                    }
                                    Err(CallToolResult::structured_error(json!({
                                        "error": message
                                    })))
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
                name: "mcp-multi-tool".into(),
                title: Some("MCP MultiTool".into()),
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
