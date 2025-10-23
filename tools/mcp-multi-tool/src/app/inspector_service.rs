use anyhow::{Context, Result};
use futures::StreamExt;
use rmcp::{
    ClientHandler, RoleClient, ServiceExt,
    handler::client::progress::ProgressDispatcher,
    model::*,
    service::PeerRequestOptions,
    transport::{
        child_process::TokioChildProcess, sse_client::SseClientTransport,
        streamable_http_client::StreamableHttpClientTransport,
    },
};
use std::{collections::BTreeMap, env, process::Stdio, time::Duration};
use tokio::{process::Command, time::timeout};

use crate::{
    infra::metrics::{LATENCY_HISTO, PendingGaugeGuard},
    shared::{
        types::{
            CallRequest, DescribeRequest, HttpTarget, ProbeRequest, ProbeResult, SseTarget,
            StreamEvent, TargetTransportKind,
        },
        utils::{measure_latency, parse_command},
    },
};

#[derive(Clone, Default)]
pub struct InspectorService;

#[derive(Clone, Default)]
struct InspectorClient {
    progress_handler: ProgressDispatcher,
}

impl InspectorClient {
    fn new() -> Self {
        Self {
            progress_handler: ProgressDispatcher::new(),
        }
    }

    fn dispatcher(&self) -> ProgressDispatcher {
        self.progress_handler.clone()
    }
}

#[derive(Debug, Clone)]
pub struct CallOutcome {
    pub result: CallToolResult,
    pub stream_events: Option<Vec<StreamEvent>>,
}

impl CallOutcome {
    fn from_result(result: CallToolResult) -> Self {
        Self {
            result,
            stream_events: None,
        }
    }

    fn with_stream(result: CallToolResult, events: Vec<StreamEvent>) -> Self {
        Self {
            result,
            stream_events: Some(events),
        }
    }
}

impl ClientHandler for InspectorClient {
    fn on_progress(
        &self,
        params: ProgressNotificationParam,
        _context: rmcp::service::NotificationContext<RoleClient>,
    ) -> impl std::future::Future<Output = ()> + Send + '_ {
        self.progress_handler.handle_notification(params)
    }
}

impl InspectorService {
    pub fn new() -> Self {
        Self
    }

    pub async fn probe(&self, req: ProbeRequest) -> Result<ProbeResult> {
        let transport = req.transport.clone().unwrap_or(TargetTransportKind::Stdio);
        match transport {
            TargetTransportKind::Stdio => self.probe_stdio(req).await,
            TargetTransportKind::Sse => self.probe_sse(req).await,
            TargetTransportKind::Http => self.probe_http(req).await,
        }
    }

    pub async fn list_tools(&self, req: ProbeRequest) -> Result<Vec<Tool>> {
        let transport = req.transport.clone().unwrap_or(TargetTransportKind::Stdio);
        match transport {
            TargetTransportKind::Stdio => {
                let (command, args) = resolve_stdio_invocation(&req)?;
                self.list_tools_stdio(command, args, req.env.clone(), req.cwd.clone())
                    .await
            }
            TargetTransportKind::Sse => {
                let target = build_sse_target(&req)?;
                self.list_tools_sse(&target).await
            }
            TargetTransportKind::Http => {
                let target = build_http_target(&req)?;
                self.list_tools_http(&target).await
            }
        }
    }

    pub async fn describe(&self, req: DescribeRequest) -> Result<Tool> {
        let tools = self.list_tools(req.probe).await?;
        let tool_name = req.tool_name;
        tools
            .into_iter()
            .find(|tool| tool.name.as_ref() == tool_name)
            .ok_or_else(|| anyhow::anyhow!("tool '{}' not found", tool_name))
    }

    async fn probe_stdio(&self, req: ProbeRequest) -> Result<ProbeResult> {
        let (program, args) = match (&req.command, &req.args) {
            (Some(cmd), Some(args)) if !cmd.is_empty() => (cmd.clone(), args.clone()),
            (Some(cmd), None) => parse_command(cmd)?,
            _ => {
                return Ok(ProbeResult {
                    ok: false,
                    transport: "stdio".into(),
                    server_name: None,
                    version: None,
                    latency_ms: None,
                    error: Some("missing command for stdio".into()),
                });
            }
        };
        let mut cmd = Command::new(program);
        cmd.args(args);
        if let Some(env) = &req.env {
            for (k, v) in env {
                cmd.env(k, v);
            }
        }
        if let Some(cwd) = &req.cwd {
            cmd.current_dir(cwd);
        }
        cmd.stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit());

        let _pending = PendingGaugeGuard::new();
        let transport = TokioChildProcess::new(cmd)?;
        let handshake_timeout = Duration::from_millis(req.handshake_timeout_ms.unwrap_or(15_000));
        let (client, latency_ms) = measure_latency(|| async move {
            let svc = timeout(handshake_timeout, ().serve(transport))
                .await
                .map_err(|_| {
                    anyhow::anyhow!(
                        "stdio handshake timed out after {} ms",
                        handshake_timeout.as_millis()
                    )
                })?
                .context("spawn stdio target")?;
            Ok::<_, anyhow::Error>(svc)
        })
        .await?;
        LATENCY_HISTO.observe(latency_ms as f64);

        // get_info may be optional; try list_tools to poke server
        let version = client.peer_info().map(|i| i.server_info.version.clone());

        Ok(ProbeResult {
            ok: true,
            transport: "stdio".into(),
            server_name: None,
            version,
            latency_ms: Some(latency_ms),
            error: None,
        })
    }

    async fn probe_sse(&self, req: ProbeRequest) -> Result<ProbeResult> {
        let url = req.url.clone().unwrap_or_default();
        if url.is_empty() {
            return Ok(ProbeResult {
                ok: false,
                transport: "sse".into(),
                server_name: None,
                version: None,
                latency_ms: None,
                error: Some("missing url".into()),
            });
        }
        // rmcp 0.8.1: the public SSE API cannot pass auth_token to start(); see help limitations
        let handshake_timeout = Duration::from_millis(req.handshake_timeout_ms.unwrap_or(15_000));
        let _pending = PendingGaugeGuard::new();
        let transport = SseClientTransport::start(url).await?;
        let (client, latency_ms) = measure_latency(|| async move {
            let svc = timeout(handshake_timeout, ().serve(transport))
                .await
                .map_err(|_| {
                    anyhow::anyhow!(
                        "sse handshake timed out after {} ms",
                        handshake_timeout.as_millis()
                    )
                })?
                .context("connect sse target")?;
            Ok::<_, anyhow::Error>(svc)
        })
        .await?;
        LATENCY_HISTO.observe(latency_ms as f64);
        let version = client.peer_info().map(|i| i.server_info.version.clone());
        Ok(ProbeResult {
            ok: true,
            transport: "sse".into(),
            server_name: None,
            version,
            latency_ms: Some(latency_ms),
            error: None,
        })
    }

    async fn probe_http(&self, req: ProbeRequest) -> Result<ProbeResult> {
        let url = req.url.clone().unwrap_or_default();
        if url.is_empty() {
            return Ok(ProbeResult {
                ok: false,
                transport: "http".into(),
                server_name: None,
                version: None,
                latency_ms: None,
                error: Some("missing url".into()),
            });
        }
        // Allow Bearer tokens for HTTP via the request config
        let mut cfg =
            rmcp::transport::streamable_http_client::StreamableHttpClientTransportConfig::with_uri(
                url.clone(),
            );
        if let Some(tok) = &req.auth_token {
            cfg = cfg.auth_header(tok);
        }
        let _pending = PendingGaugeGuard::new();
        let transport = StreamableHttpClientTransport::with_client(reqwest::Client::new(), cfg);
        let handshake_timeout = Duration::from_millis(req.handshake_timeout_ms.unwrap_or(15_000));
        let (client, latency_ms) = measure_latency(|| async move {
            let svc = timeout(handshake_timeout, ().serve(transport))
                .await
                .map_err(|_| {
                    anyhow::anyhow!(
                        "http handshake timed out after {} ms",
                        handshake_timeout.as_millis()
                    )
                })?
                .context("connect http target")?;
            Ok::<_, anyhow::Error>(svc)
        })
        .await?;
        LATENCY_HISTO.observe(latency_ms as f64);
        let version = client.peer_info().map(|i| i.server_info.version.clone());
        Ok(ProbeResult {
            ok: true,
            transport: "http".into(),
            server_name: None,
            version,
            latency_ms: Some(latency_ms),
            error: None,
        })
    }

    pub async fn list_tools_stdio(
        &self,
        command: String,
        args: Vec<String>,
        env: Option<BTreeMap<String, String>>,
        cwd: Option<String>,
    ) -> Result<Vec<Tool>> {
        let _pending = PendingGaugeGuard::new();
        let mut cmd = Command::new(command);
        cmd.args(args);
        if let Some(env) = env {
            for (k, v) in env {
                cmd.env(k, v);
            }
        }
        if let Some(cwd) = cwd {
            cmd.current_dir(cwd);
        }
        let client = ().serve(TokioChildProcess::new(cmd)?).await?;
        let tools = client.list_tools(Default::default()).await?.tools;
        Ok(tools)
    }

    pub async fn list_tools_sse(&self, target: &SseTarget) -> Result<Vec<Tool>> {
        let _pending = PendingGaugeGuard::new();
        let url = target.url.clone();
        if url.is_empty() {
            anyhow::bail!("missing sse url");
        }
        let handshake_timeout =
            Duration::from_millis(target.handshake_timeout_ms.unwrap_or(15_000));
        let transport = SseClientTransport::start(url).await?;
        let client = timeout(handshake_timeout, ().serve(transport))
            .await
            .map_err(|_| anyhow::anyhow!("sse handshake timed out"))??;
        let tools = client.list_tools(Default::default()).await?.tools;
        Ok(tools)
    }

    pub async fn list_tools_http(&self, target: &HttpTarget) -> Result<Vec<Tool>> {
        let _pending = PendingGaugeGuard::new();
        let url = target.url.clone();
        if url.is_empty() {
            anyhow::bail!("missing http url");
        }
        let mut cfg =
            rmcp::transport::streamable_http_client::StreamableHttpClientTransportConfig::with_uri(
                url.clone(),
            );
        if let Some(tok) = &target.auth_token {
            cfg = cfg.auth_header(tok);
        }
        let transport = StreamableHttpClientTransport::with_client(reqwest::Client::new(), cfg);
        let handshake_timeout =
            Duration::from_millis(target.handshake_timeout_ms.unwrap_or(15_000));
        let client = timeout(handshake_timeout, ().serve(transport))
            .await
            .map_err(|_| anyhow::anyhow!("http handshake timed out"))??;
        let tools = client.list_tools(Default::default()).await?.tools;
        Ok(tools)
    }

    pub async fn call_stdio(
        &self,
        command: String,
        args: Vec<String>,
        env: Option<BTreeMap<String, String>>,
        cwd: Option<String>,
        request: &CallRequest,
    ) -> Result<CallOutcome> {
        let _pending = PendingGaugeGuard::new();
        let mut cmd = Command::new(command);
        cmd.args(args);
        if let Some(env) = env {
            for (k, v) in env {
                cmd.env(k, v);
            }
        }
        if let Some(cwd) = cwd {
            cmd.current_dir(cwd);
        }
        let handler = InspectorClient::new();
        let client = handler.serve(TokioChildProcess::new(cmd)?).await?;
        self.invoke_call(client, request).await
    }

    pub async fn call_sse(&self, target: &SseTarget, request: &CallRequest) -> Result<CallOutcome> {
        let _pending = PendingGaugeGuard::new();
        let url = target.url.clone();
        if url.is_empty() {
            anyhow::bail!("missing sse url");
        }
        let handshake_timeout =
            Duration::from_millis(target.handshake_timeout_ms.unwrap_or(15_000));
        let handler = InspectorClient::new();
        let transport = SseClientTransport::start(url).await?;
        let client = timeout(handshake_timeout, handler.serve(transport))
            .await
            .map_err(|_| anyhow::anyhow!("sse handshake timed out"))??;
        self.invoke_call(client, request).await
    }

    pub async fn call_http(
        &self,
        target: &HttpTarget,
        request: &CallRequest,
    ) -> Result<CallOutcome> {
        let _pending = PendingGaugeGuard::new();
        let url = target.url.clone();
        if url.is_empty() {
            anyhow::bail!("missing http url");
        }
        let mut cfg =
            rmcp::transport::streamable_http_client::StreamableHttpClientTransportConfig::with_uri(
                url.clone(),
            );
        if let Some(tok) = &target.auth_token {
            cfg = cfg.auth_header(tok);
        }
        let transport = StreamableHttpClientTransport::with_client(reqwest::Client::new(), cfg);
        let handshake_timeout =
            Duration::from_millis(target.handshake_timeout_ms.unwrap_or(15_000));
        let handler = InspectorClient::new();
        let client = timeout(handshake_timeout, handler.serve(transport))
            .await
            .map_err(|_| anyhow::anyhow!("http handshake timed out"))??;
        self.invoke_call(client, request).await
    }
}

impl InspectorService {
    async fn invoke_call(
        &self,
        client: rmcp::service::RunningService<RoleClient, InspectorClient>,
        request: &CallRequest,
    ) -> Result<CallOutcome> {
        let params = CallToolRequestParam {
            name: request.tool_name.clone().into(),
            arguments: request.arguments_json.as_object().cloned(),
        };
        if request.stream {
            self.call_with_stream(client, params).await
        } else {
            let res = client.call_tool(params).await?;
            Ok(CallOutcome::from_result(res))
        }
    }

    async fn call_with_stream(
        &self,
        client: rmcp::service::RunningService<RoleClient, InspectorClient>,
        params: CallToolRequestParam,
    ) -> Result<CallOutcome> {
        let dispatcher = client.service().dispatcher();
        let handle = client
            .send_cancellable_request(
                ClientRequest::CallToolRequest(Request::new(params)),
                PeerRequestOptions::no_options(),
            )
            .await?;
        let progress_token = handle.progress_token.clone();
        let mut progress_stream = dispatcher.subscribe(progress_token).await;

        let response = handle.await_response().await?;
        let mut final_result = match response {
            ServerResult::CallToolResult(result) => result,
            other => {
                return Err(anyhow::anyhow!("unexpected server response: {:?}", other));
            }
        };

        let mut events: Vec<StreamEvent> = Vec::new();
        loop {
            match tokio::time::timeout(Duration::from_millis(25), progress_stream.next()).await {
                Ok(Some(progress)) => events.push(progress_to_event(progress)),
                Ok(None) => break,
                Err(_) => break,
            }
        }

        events.push(result_to_event(&final_result));
        let final_snapshot = serde_json::to_value(&final_result).ok();
        let events_clone = events.clone();
        final_result.structured_content = Some(serde_json::json!({
            "mode": "stream",
            "events": events_clone,
            "final": final_snapshot,
        }));

        Ok(CallOutcome::with_stream(final_result, events))
    }
}

fn progress_to_event(progress: ProgressNotificationParam) -> StreamEvent {
    StreamEvent {
        event: "chunk".into(),
        progress: Some(progress.progress),
        total: progress.total,
        message: progress.message,
        structured: None,
        content: None,
        error: None,
    }
}

fn result_to_event(result: &CallToolResult) -> StreamEvent {
    let is_error = result.is_error.unwrap_or(false);
    StreamEvent {
        event: if is_error {
            "error".into()
        } else {
            "final".into()
        },
        progress: None,
        total: None,
        message: None,
        structured: result.structured_content.clone(),
        content: serde_json::to_value(&result.content).ok(),
        error: if is_error {
            Some("tool execution failed".into())
        } else {
            None
        },
    }
}

fn resolve_stdio_invocation(req: &ProbeRequest) -> Result<(String, Vec<String>)> {
    if let Some(cmd) = req.command.as_ref() {
        if cmd.trim().is_empty() {
            return Err(anyhow::anyhow!("command is required for stdio transport"));
        }
        if let Some(args) = req.args.as_ref() {
            return Ok((cmd.clone(), args.clone()));
        }
        return parse_command(cmd);
    }
    if let Some(args) = req.args.as_ref() {
        if !args.is_empty() {
            return Err(anyhow::anyhow!(
                "arguments provided without command for stdio transport"
            ));
        }
    }
    let env_cmd = env::var("INSPECTOR_STDIO_CMD").map_err(|_| {
        anyhow::anyhow!(
            "command is required for stdio transport; set 'command'/'args' or INSPECTOR_STDIO_CMD"
        )
    })?;
    parse_command(&env_cmd)
}

fn build_sse_target(req: &ProbeRequest) -> Result<SseTarget> {
    let url = req.url.clone().unwrap_or_default();
    if url.is_empty() {
        return Err(anyhow::anyhow!("missing sse url"));
    }
    Ok(SseTarget {
        url,
        headers: req.headers.clone(),
        handshake_timeout_ms: req.handshake_timeout_ms,
    })
}

fn build_http_target(req: &ProbeRequest) -> Result<HttpTarget> {
    let url = req.url.clone().unwrap_or_default();
    if url.is_empty() {
        return Err(anyhow::anyhow!("missing http url"));
    }
    Ok(HttpTarget {
        url,
        headers: req.headers.clone(),
        auth_token: req.auth_token.clone(),
        handshake_timeout_ms: req.handshake_timeout_ms,
    })
}
