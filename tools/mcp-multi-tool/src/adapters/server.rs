use anyhow::Result;
use rmcp::{ErrorData as McpError, ServerHandler, model::*};
use serde_json::{Value, json};
use std::{
    sync::Arc,
    time::{Instant, SystemTime},
};
use time::{OffsetDateTime, format_description::well_known::Rfc3339};

use crate::{
    app::{
        error_budget::{ErrorBudget, FreezeReport, RecordOutcome},
        inspector_service::{CallOutcome, InspectorService},
        registry::ToolRegistry,
    },
    domain::run::{InspectionRun, RunState},
    infra::{config::IdempotencyConflictPolicy, metrics, outbox::Outbox},
    shared::{
        idempotency::{ClaimOutcome, IdempotencyStore},
        types::{
            CallRequest, CallTrace, DescribeRequest, InspectionRunEvent, ProbeRequest,
            TargetDescriptor,
        },
    },
};

#[derive(Clone)]
pub struct InspectorServer {
    svc: InspectorService,
    registry: ToolRegistry,
    outbox: Arc<Outbox>,
    idempotency: Arc<IdempotencyStore>,
    conflict_policy: IdempotencyConflictPolicy,
    error_budget: Arc<ErrorBudget>,
}

impl InspectorServer {
    pub fn new(
        svc: InspectorService,
        registry: ToolRegistry,
        outbox: Arc<Outbox>,
        idempotency: Arc<IdempotencyStore>,
        conflict_policy: IdempotencyConflictPolicy,
        error_budget: Arc<ErrorBudget>,
    ) -> Self {
        Self {
            svc,
            registry,
            outbox,
            idempotency,
            conflict_policy,
            error_budget,
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
        external_reference: Option<String>,
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
            idempotency_key: request.idempotency_key.clone(),
            external_reference,
        }
    }

    fn snapshot_result(&self, result: &CallToolResult) -> Option<Value> {
        serde_json::to_value(result).ok()
    }

    fn attach_trace(result: &mut CallToolResult, trace: &CallTrace) {
        match serde_json::to_value(trace) {
            Ok(value) => {
                let mut meta = result.meta.take().unwrap_or_else(Meta::new);
                meta.insert("trace".into(), value);
                result.meta = Some(meta);
            }
            Err(err) => {
                tracing::error!(%err, "failed to serialize call trace");
            }
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

            let release_track = this.registry.release_track();
            if !release_track.allows_inspector() && name != "help" && name != "inspector_help" {
                let payload = serde_json::json!({
                    "error": "inspector disabled by release track",
                    "code": "RELEASE_TRACK_ROLLBACK"
                });
                run.fail();
                return Ok(CallToolResult::structured_error(payload));
            }

            let result: Result<CallToolResult, CallToolResult> = match name {
                "help" | "inspector_help" => {
                    let mut lines = vec![
                        serde_json::json!({
                            "section": "summary",
                            "server": "mcp-multi-tool",
                            "version": env!("CARGO_PKG_VERSION"),
                            "protocol": "MCP",
                            "release_track": release_track.as_str(),
                            "transports": ["stdio", "sse", "http"]
                        }),
                        serde_json::json!({
                            "section": "tool",
                            "name": "inspector_probe",
                            "summary": "Probe a downstream MCP server and measure latency.",
                            "arguments": {
                                "transport": "string stdio|sse|http",
                                "command": "optional string",
                                "args": "optional array<string>",
                                "env": "optional map",
                                "cwd": "optional string",
                                "url": "optional string",
                                "headers": "optional map",
                                "auth_token": "optional string",
                                "handshake_timeout_ms": "optional int"
                            },
                            "returns": "ProbeResult"
                        }),
                        serde_json::json!({
                            "section": "tool",
                            "name": "inspector_list_tools",
                            "summary": "List tools exposed by the target MCP.",
                            "arguments": {
                                "transport": "string stdio|sse|http",
                                "command": "optional string",
                                "args": "optional array<string>",
                                "env": "optional map",
                                "cwd": "optional string",
                                "url": "optional string",
                                "headers": "optional map",
                                "auth_token": "optional string",
                                "handshake_timeout_ms": "optional int"
                            },
                            "returns": "array<Tool>"
                        }),
                        serde_json::json!({
                            "section": "tool",
                            "name": "inspector_describe",
                            "summary": "Fetch JSON schema and annotations for a tool.",
                            "arguments": {
                                "tool_name": "string",
                                "transport": "optional string",
                                "command": "optional string",
                                "args": "optional array<string>",
                                "env": "optional map",
                                "cwd": "optional string",
                                "url": "optional string",
                                "headers": "optional map",
                                "auth_token": "optional string",
                                "handshake_timeout_ms": "optional int"
                            },
                            "returns": "Tool"
                        }),
                        serde_json::json!({
                            "section": "tool",
                            "name": "inspector_call",
                            "summary": "Invoke a downstream tool with optional streaming.",
                            "arguments": {
                                "tool_name": "string",
                                "arguments_json": "object",
                                "idempotency_key": "optional string",
                                "external_reference": "optional string",
                                "stream": "boolean",
                                "stdio": "optional target",
                                "sse": "optional target",
                                "http": "optional target"
                            },
                            "returns": "CallToolResult",
                            "notes": [
                                "Set stream=true to capture progress notifications.",
                                "When the error budget is exhausted the server returns ERROR_BUDGET_EXHAUSTED until the success rate recovers."
                            ]
                        }),
                        serde_json::json!({
                            "section": "environment",
                            "INSPECTOR_STDIO_CMD": "<command> [args...] required when no stdio target override is provided",
                            "ERROR_BUDGET_*": "tune freeze threshold (see docs/howto/onboarding.md)",
                            "RUST_LOG": "default info"
                        }),
                        serde_json::json!({
                            "section": "workflow",
                            "steps": [
                                "inspector_probe",
                                "inspector_list_tools",
                                "inspector_describe",
                                "inspector_call"
                            ],
                            "diagnostics": [
                                "Prometheus /metrics -> inspector_lock_wait_ms histogram",
                                "Outbox JSONL/SQLite at data/outbox"
                            ]
                        }),
                    ];

                    if !release_track.allows_inspector() {
                        lines.push(serde_json::json!({
                            "section": "notice",
                            "code": "release_track_rollback",
                            "message": "Inspector tools temporarily disabled; set RELEASE_TRACK=stable or canary to re-enable."
                        }));
                    }

                    let payload = serde_json::json!({
                        "format": "jsonl",
                        "lines": lines
                            .into_iter()
                            .map(|entry| serde_json::to_string(&entry).unwrap())
                            .collect::<Vec<_>>()
                    });
                    Ok(CallToolResult::structured(payload))
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
                    match serde_json::from_value::<ProbeRequest>(args_val) {
                        Ok(req) => match this.svc.list_tools(req).await {
                            Ok(tools) => Ok(CallToolResult::structured(json!({
                                "tools": tools
                            }))),
                            Err(e) => Err(failure(&e.to_string())),
                        },
                        Err(e) => Err(failure(&e.to_string())),
                    }
                }
                "inspector_describe" | "inspector.describe" => {
                    match serde_json::from_value::<DescribeRequest>(args_val) {
                        Ok(req) => match this.svc.describe(req).await {
                            Ok(tool) => Ok(CallToolResult::structured(json!({
                                "tool": tool
                            }))),
                            Err(e) => Err(failure(&e.to_string())),
                        },
                        Err(e) => Err(failure(&e.to_string())),
                    }
                }
                "inspector_call" | "inspector.call" => {
                    match serde_json::from_value::<CallRequest>(args_val) {
                        Ok(req) => {
                            let started_at = OffsetDateTime::now_utc();
                            let timer = Instant::now();
                            let admit_clock = SystemTime::now();
                            let mut target_descriptor = TargetDescriptor {
                                transport: "stdio".into(),
                                command: None,
                                url: None,
                                headers: None,
                            };
                            let mut external_reference = req.external_reference.clone();
                            if let Some(ref ext) = external_reference {
                                if let Some(existing) = this.idempotency.find_external_ref(ext) {
                                    return match this.conflict_policy {
                                        IdempotencyConflictPolicy::ReturnExisting => {
                                            run.capture();
                                            Ok(this.return_existing_event(existing))
                                        }
                                        IdempotencyConflictPolicy::Conflict409 => {
                                            run.fail();
                                            Ok(this.idempotency_conflict_response(
                                                Some(existing),
                                                "external reference conflict",
                                            ))
                                        }
                                    };
                                }
                            }
                            let mut claimed_key: Option<String> = None;
                            if let Some(key) = req.idempotency_key.clone() {
                                match this.idempotency.claim(&key) {
                                    ClaimOutcome::Accepted => {
                                        this.idempotency.begin(&key, run_id, &req);
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
                            if let Some(key) = claimed_key.as_ref() {
                                this.idempotency.mark_started(key, started_at);
                            }
                            match this.error_budget.admit(admit_clock) {
                                Ok(thawed) => {
                                    if thawed {
                                        metrics::set_error_budget_frozen(false);
                                    }
                                }
                                Err(report) => {
                                    metrics::set_error_budget_frozen(true);
                                    run.fail();
                                    let duration_ms = timer.elapsed().as_millis() as u64;
                                    let payload = freeze_payload(&report);
                                    let event = this.build_event(
                                        &run,
                                        &req,
                                        started_at,
                                        duration_ms,
                                        None,
                                        None,
                                        Some("error budget exhausted".into()),
                                        external_reference.clone(),
                                    );
                                    if let Err(e) = this.outbox.append(&event) {
                                        tracing::error!(%run_id, error=%e, "failed to append freeze event to outbox");
                                    }
                                    if let Some(ref ext) = external_reference {
                                        this.idempotency.record_external_ref(ext, event.clone());
                                    }
                                    if let Some(key) = claimed_key {
                                        this.idempotency.complete(&key, event.clone());
                                    }
                                    tracing::warn!(%run_id, "error budget freeze active");
                                    return Ok(CallToolResult::structured_error(payload));
                                }
                            }
                            let call_result = if let Some(http) = req.http.as_ref() {
                                target_descriptor.transport = "http".into();
                                target_descriptor.url = Some(http.url.clone());
                                target_descriptor.headers = http.headers.clone();
                                if let Some(key) = claimed_key.as_ref() {
                                    this.idempotency.set_target(key, target_descriptor.clone());
                                }
                                this.svc.call_http(http, &req).await
                            } else if let Some(sse) = req.sse.as_ref() {
                                target_descriptor.transport = "sse".into();
                                target_descriptor.url = Some(sse.url.clone());
                                target_descriptor.headers = sse.headers.clone();
                                if let Some(key) = claimed_key.as_ref() {
                                    this.idempotency.set_target(key, target_descriptor.clone());
                                }
                                this.svc.call_sse(sse, &req).await
                            } else if let Some(target) = req.stdio.as_ref() {
                                target_descriptor.transport = "stdio".into();
                                target_descriptor.command = Some(target.command.clone());
                                if let Some(key) = claimed_key.as_ref() {
                                    this.idempotency.set_target(key, target_descriptor.clone());
                                }
                                this.svc
                                    .call_stdio(
                                        target.command.clone(),
                                        target.args.clone(),
                                        target.env.clone(),
                                        target.cwd.clone(),
                                        &req,
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
                                                if let Some(key) = claimed_key.as_ref() {
                                                    this.idempotency
                                                        .set_target(key, target_descriptor.clone());
                                                }
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
                                            .call_stdio(program.clone(), args, None, None, &req)
                                            .await
                                    }
                                    Err(err) => return Ok(err),
                                }
                            };
                            match call_result {
                                Ok(CallOutcome {
                                    mut result,
                                    stream_events,
                                }) => {
                                    if matches!(run.state, RunState::Processing) {
                                        run.capture();
                                    } else {
                                        tracing::warn!(
                                            state = run.state.as_str(),
                                            "run not in processing state at success"
                                        );
                                    }
                                    let duration_ms = timer.elapsed().as_millis() as u64;
                                    if let Some(meta_ref) = extract_external_reference(&result) {
                                        external_reference = Some(meta_ref);
                                    }
                                    let event = this.build_event(
                                        &run,
                                        &req,
                                        started_at,
                                        duration_ms,
                                        Some(target_descriptor),
                                        this.snapshot_result(&result),
                                        None,
                                        external_reference.clone(),
                                    );
                                    let outbox_result = this.outbox.append(&event);
                                    let outbox_persisted = outbox_result.is_ok();
                                    if let Err(e) = outbox_result {
                                        tracing::error!(%run_id, error=%e, "failed to append outbox event");
                                    }
                                    if let Some(ref ext) = external_reference {
                                        this.idempotency.record_external_ref(ext, event.clone());
                                    }
                                    if let Some(key) = claimed_key {
                                        this.idempotency.complete(&key, event.clone());
                                    }
                                    let trace = CallTrace {
                                        event: event.clone(),
                                        stream_enabled: req.stream,
                                        stream_events,
                                        outbox_persisted,
                                    };
                                    Self::attach_trace(&mut result, &trace);
                                    match this.error_budget.record_success_now() {
                                        RecordOutcome::FreezeTriggered(report) => {
                                            metrics::set_error_budget_frozen(true);
                                            tracing::warn!(%run_id, success_rate = report.success_rate, sample_size = report.sample_size, "error budget freeze triggered on success");
                                        }
                                        RecordOutcome::FreezeCleared => {
                                            metrics::set_error_budget_frozen(false);
                                            tracing::info!(%run_id, "error budget freeze lifted");
                                        }
                                        RecordOutcome::None => {}
                                    }
                                    Ok(result)
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
                                        external_reference.clone(),
                                    );
                                    let outbox_result = this.outbox.append(&event);
                                    let outbox_persisted = outbox_result.is_ok();
                                    if let Err(e) = outbox_result {
                                        tracing::error!(%run_id, error=%e, "failed to append failed event to outbox");
                                    }
                                    if let Some(ref ext) = external_reference {
                                        this.idempotency.record_external_ref(ext, event.clone());
                                    }
                                    if let Some(key) = claimed_key {
                                        this.idempotency.complete(&key, event.clone());
                                    }
                                    let mut err_result = CallToolResult::structured_error(json!({
                                        "error": message
                                    }));
                                    let trace = CallTrace {
                                        event: event.clone(),
                                        stream_enabled: req.stream,
                                        stream_events: None,
                                        outbox_persisted,
                                    };
                                    Self::attach_trace(&mut err_result, &trace);
                                    match this.error_budget.record_failure_now() {
                                        RecordOutcome::FreezeTriggered(report) => {
                                            metrics::set_error_budget_frozen(true);
                                            tracing::warn!(%run_id, success_rate = report.success_rate, sample_size = report.sample_size, "error budget freeze triggered");
                                        }
                                        RecordOutcome::FreezeCleared => {
                                            metrics::set_error_budget_frozen(false);
                                            tracing::info!(%run_id, "error budget freeze lifted");
                                        }
                                        RecordOutcome::None => {}
                                    }
                                    Err(err_result)
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

fn extract_external_reference(result: &CallToolResult) -> Option<String> {
    result.meta.as_ref().and_then(|meta| {
        meta.get("externalReference")
            .or_else(|| meta.get("external_reference"))
            .and_then(|value| value.as_str().map(|s| s.to_string()))
    })
}

fn freeze_payload(report: &FreezeReport) -> serde_json::Value {
    let until_dt = OffsetDateTime::from(report.until);
    let until = until_dt
        .format(&Rfc3339)
        .unwrap_or_else(|_| "1970-01-01T00:00:00Z".into());
    json!({
        "error": "error budget exhausted",
        "code": "ERROR_BUDGET_EXHAUSTED",
        "frozen_until": until,
        "success_rate": report.success_rate,
        "sample_size": report.sample_size,
    })
}
