use anyhow::Result;
use serde::Serialize;
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    time::{Duration, Instant},
};
use time::OffsetDateTime;
use tokio::time::sleep;

use crate::{
    app::inspector_service::InspectorService,
    shared::types::{
        CallRequest, DescribeRequest, HttpTarget, ProbeRequest, SseTarget, TargetTransportKind,
    },
};

#[derive(Clone, Debug, Serialize, Default)]
pub struct ComplianceTarget {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub env: Option<BTreeMap<String, String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cwd: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sse_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub http_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub http_headers: Option<BTreeMap<String, String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub http_auth_token: Option<String>,
}

impl ComplianceTarget {
    pub fn stdio<S: Into<String>>(command: S) -> Self {
        Self {
            command: Some(command.into()),
            ..Default::default()
        }
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct CaseResult {
    pub name: String,
    pub passed: bool,
    pub duration_ms: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<Value>,
}

#[derive(Clone, Debug, Serialize)]
pub struct ComplianceReport {
    pub started_at: String,
    pub finished_at: String,
    pub pass_rate: f64,
    pub cases: Vec<CaseResult>,
}

impl ComplianceReport {
    pub fn passed(&self) -> bool {
        self.pass_rate >= 0.95
    }

    pub fn to_markdown(&self) -> String {
        let mut md = String::new();
        md.push_str("| Case | Status | Duration (ms) | Notes |\n");
        md.push_str("| --- | --- | --- | --- |\n");
        for case in &self.cases {
            let status = if case.passed { "✅" } else { "❌" };
            let notes = case
                .detail
                .as_ref()
                .map(|v| serde_json::to_string(v).unwrap_or_default())
                .unwrap_or_else(|| "-".into());
            md.push_str(&format!(
                "| {} | {} | {} | {} |\n",
                case.name, status, case.duration_ms, notes
            ));
        }
        md.push_str(&format!(
            "\nPass rate: {:.2}% (threshold 95%)",
            self.pass_rate * 100.0
        ));
        md
    }
}

pub struct ComplianceSuite {
    svc: InspectorService,
}

impl Default for ComplianceSuite {
    fn default() -> Self {
        Self {
            svc: InspectorService::new(),
        }
    }
}

impl ComplianceSuite {
    pub fn new() -> Self {
        Self::default()
    }

    pub async fn run(&self, target: ComplianceTarget) -> Result<ComplianceReport> {
        let started_at = OffsetDateTime::now_utc();
        let mut cases = Vec::new();

        if let Some(case) = self.probe_stdio_case(&target).await? {
            cases.push(case);
        }
        if let Some(case) = self.list_tools_stdio_case(&target).await? {
            cases.push(case);
        }
        if let Some(case) = self.list_tools_sse_case(&target).await? {
            cases.push(case);
        }
        if let Some(case) = self.list_tools_http_case(&target).await? {
            cases.push(case);
        }
        if let Some(case) = self.describe_stdio_case(&target).await? {
            cases.push(case);
        }
        if let Some(case) = self.describe_sse_case(&target).await? {
            cases.push(case);
        }
        if let Some(case) = self.describe_http_case(&target).await? {
            cases.push(case);
        }
        if target.sse_url.is_some() || target.http_url.is_some() {
            sleep(Duration::from_millis(200)).await;
        }
        if let Some(case) = self.call_stdio_case(&target).await? {
            cases.push(case);
        }
        if let Some(case) = self.call_stdio_stream_case(&target).await? {
            cases.push(case);
        }
        if let Some(case) = self.call_sse_case(&target).await? {
            cases.push(case);
        }
        if let Some(case) = self.call_http_case(&target).await? {
            cases.push(case);
        }
        if let Some(sse_case) = self.probe_sse_case(&target).await? {
            cases.push(sse_case);
        }
        if let Some(http_case) = self.probe_http_case(&target).await? {
            cases.push(http_case);
        }
        cases.push(self.missing_command_case().await?);

        let finished_at = OffsetDateTime::now_utc();
        let pass_count = cases.iter().filter(|c| c.passed).count() as f64;
        let total = cases.len().max(1) as f64;
        let pass_rate = pass_count / total;

        Ok(ComplianceReport {
            started_at: started_at.to_string(),
            finished_at: finished_at.to_string(),
            pass_rate,
            cases,
        })
    }

    async fn probe_stdio_case(&self, target: &ComplianceTarget) -> Result<Option<CaseResult>> {
        let Some(command) = target.command.as_ref() else {
            return Ok(None);
        };
        let timer = Instant::now();
        let req = ProbeRequest {
            transport: Some(TargetTransportKind::Stdio),
            command: Some(command.clone()),
            args: Some(target.args.clone()),
            env: target.env.clone(),
            cwd: target.cwd.clone(),
            url: None,
            headers: None,
            auth_token: None,
            handshake_timeout_ms: Some(15_000),
        };
        match self.svc.probe(req).await {
            Ok(res) => {
                let passed = res.ok;
                Ok(Some(CaseResult {
                    name: "probe_stdio".into(),
                    passed,
                    duration_ms: timer.elapsed().as_millis() as u64,
                    detail: Some(json!({
                        "transport": res.transport,
                        "version": res.version,
                        "latency_ms": res.latency_ms,
                        "error": res.error,
                    })),
                }))
            }
            Err(err) => Ok(Some(CaseResult {
                name: "probe_stdio".into(),
                passed: false,
                duration_ms: timer.elapsed().as_millis() as u64,
                detail: Some(json!({
                    "error": err.to_string()
                })),
            })),
        }
    }

    async fn list_tools_stdio_case(&self, target: &ComplianceTarget) -> Result<Option<CaseResult>> {
        let Some(command) = target.command.as_ref() else {
            return Ok(None);
        };
        let timer = Instant::now();
        let outcome = self
            .svc
            .list_tools_stdio(
                command.clone(),
                target.args.clone(),
                target.env.clone(),
                target.cwd.clone(),
            )
            .await;
        match outcome {
            Ok(tools) => {
                let passed = !tools.is_empty();
                Ok(Some(CaseResult {
                    name: "list_tools".into(),
                    passed,
                    duration_ms: timer.elapsed().as_millis() as u64,
                    detail: Some(json!({
                        "tool_count": tools.len(),
                    })),
                }))
            }
            Err(err) => Ok(Some(CaseResult {
                name: "list_tools".into(),
                passed: false,
                duration_ms: timer.elapsed().as_millis() as u64,
                detail: Some(json!({
                    "error": err.to_string()
                })),
            })),
        }
    }

    async fn list_tools_sse_case(&self, target: &ComplianceTarget) -> Result<Option<CaseResult>> {
        let Some(url) = target.sse_url.clone() else {
            return Ok(None);
        };
        let timer = Instant::now();
        let sse_target = SseTarget {
            url: url.clone(),
            headers: None,
            handshake_timeout_ms: Some(15_000),
        };
        let outcome = self.svc.list_tools_sse(&sse_target).await;
        Ok(Some(match outcome {
            Ok(tools) => CaseResult {
                name: "list_tools_sse".into(),
                passed: !tools.is_empty(),
                duration_ms: timer.elapsed().as_millis() as u64,
                detail: Some(json!({
                    "url": url,
                    "tool_count": tools.len(),
                })),
            },
            Err(err) => CaseResult {
                name: "list_tools_sse".into(),
                passed: false,
                duration_ms: timer.elapsed().as_millis() as u64,
                detail: Some(json!({"error": err.to_string()})),
            },
        }))
    }

    async fn list_tools_http_case(&self, target: &ComplianceTarget) -> Result<Option<CaseResult>> {
        let Some(url) = target.http_url.clone() else {
            return Ok(None);
        };
        let timer = Instant::now();
        let http_target = HttpTarget {
            url: url.clone(),
            headers: target.http_headers.clone(),
            auth_token: target.http_auth_token.clone(),
            handshake_timeout_ms: Some(15_000),
        };
        let outcome = self.svc.list_tools_http(&http_target).await;
        Ok(Some(match outcome {
            Ok(tools) => CaseResult {
                name: "list_tools_http".into(),
                passed: !tools.is_empty(),
                duration_ms: timer.elapsed().as_millis() as u64,
                detail: Some(json!({
                    "url": url,
                    "tool_count": tools.len(),
                })),
            },
            Err(err) => CaseResult {
                name: "list_tools_http".into(),
                passed: false,
                duration_ms: timer.elapsed().as_millis() as u64,
                detail: Some(json!({"error": err.to_string()})),
            },
        }))
    }

    async fn describe_stdio_case(&self, target: &ComplianceTarget) -> Result<Option<CaseResult>> {
        let Some(command) = target.command.as_ref() else {
            return Ok(None);
        };
        let timer = Instant::now();
        let req = DescribeRequest {
            tool_name: "help".into(),
            probe: ProbeRequest {
                transport: Some(TargetTransportKind::Stdio),
                command: Some(command.clone()),
                args: Some(target.args.clone()),
                env: target.env.clone(),
                cwd: target.cwd.clone(),
                url: None,
                headers: None,
                auth_token: None,
                handshake_timeout_ms: Some(15_000),
            },
        };
        match self.svc.describe(req).await {
            Ok(tool) => Ok(Some(CaseResult {
                name: "describe_help".into(),
                passed: tool.name.as_ref() == "help",
                duration_ms: timer.elapsed().as_millis() as u64,
                detail: serde_json::to_value(&tool).ok().map(|v| json!({"tool": v})),
            })),
            Err(err) => Ok(Some(CaseResult {
                name: "describe_help".into(),
                passed: false,
                duration_ms: timer.elapsed().as_millis() as u64,
                detail: Some(json!({"error": err.to_string()})),
            })),
        }
    }

    async fn describe_sse_case(&self, target: &ComplianceTarget) -> Result<Option<CaseResult>> {
        let Some(url) = target.sse_url.clone() else {
            return Ok(None);
        };
        let timer = Instant::now();
        let req = DescribeRequest {
            tool_name: "help".into(),
            probe: ProbeRequest {
                transport: Some(TargetTransportKind::Sse),
                command: None,
                args: None,
                env: None,
                cwd: None,
                url: Some(url.clone()),
                headers: None,
                auth_token: None,
                handshake_timeout_ms: Some(15_000),
            },
        };
        match self.svc.describe(req).await {
            Ok(tool) => Ok(Some(CaseResult {
                name: "describe_help_sse".into(),
                passed: tool.name.as_ref() == "help",
                duration_ms: timer.elapsed().as_millis() as u64,
                detail: serde_json::to_value(&tool).ok().map(|v| json!({"tool": v})),
            })),
            Err(err) => Ok(Some(CaseResult {
                name: "describe_help_sse".into(),
                passed: false,
                duration_ms: timer.elapsed().as_millis() as u64,
                detail: Some(json!({"error": err.to_string()})),
            })),
        }
    }

    async fn describe_http_case(&self, target: &ComplianceTarget) -> Result<Option<CaseResult>> {
        let Some(url) = target.http_url.clone() else {
            return Ok(None);
        };
        let timer = Instant::now();
        let req = DescribeRequest {
            tool_name: "help".into(),
            probe: ProbeRequest {
                transport: Some(TargetTransportKind::Http),
                command: None,
                args: None,
                env: None,
                cwd: None,
                url: Some(url.clone()),
                headers: target.http_headers.clone(),
                auth_token: target.http_auth_token.clone(),
                handshake_timeout_ms: Some(15_000),
            },
        };
        match self.svc.describe(req).await {
            Ok(tool) => Ok(Some(CaseResult {
                name: "describe_help_http".into(),
                passed: tool.name.as_ref() == "help",
                duration_ms: timer.elapsed().as_millis() as u64,
                detail: serde_json::to_value(&tool).ok().map(|v| json!({"tool": v})),
            })),
            Err(err) => Ok(Some(CaseResult {
                name: "describe_help_http".into(),
                passed: false,
                duration_ms: timer.elapsed().as_millis() as u64,
                detail: Some(json!({"error": err.to_string()})),
            })),
        }
    }

    async fn call_stdio_case(&self, target: &ComplianceTarget) -> Result<Option<CaseResult>> {
        let Some(command) = target.command.as_ref() else {
            return Ok(None);
        };
        let timer = Instant::now();
        let request = CallRequest {
            tool_name: "help".into(),
            arguments_json: json!({}),
            idempotency_key: None,
            stream: false,
            external_reference: None,
            stdio: None,
            sse: None,
            http: None,
        };
        let outcome = self
            .svc
            .call_stdio(
                command.clone(),
                target.args.clone(),
                target.env.clone(),
                target.cwd.clone(),
                &request,
            )
            .await;
        match outcome {
            Ok(res) => {
                let passed = res.structured_content.is_some() || !res.content.is_empty();
                Ok(Some(CaseResult {
                    name: "call_help".into(),
                    passed,
                    duration_ms: timer.elapsed().as_millis() as u64,
                    detail: self.snapshot(&res),
                }))
            }
            Err(err) => Ok(Some(CaseResult {
                name: "call_help".into(),
                passed: false,
                duration_ms: timer.elapsed().as_millis() as u64,
                detail: Some(json!({
                    "error": err.to_string()
                })),
            })),
        }
    }

    async fn call_sse_case(&self, target: &ComplianceTarget) -> Result<Option<CaseResult>> {
        let Some(url) = target.sse_url.clone() else {
            return Ok(None);
        };
        let timer = Instant::now();
        let sse_target = SseTarget {
            url: url.clone(),
            headers: None,
            handshake_timeout_ms: Some(15_000),
        };
        let request = CallRequest {
            tool_name: "help".into(),
            arguments_json: json!({}),
            idempotency_key: None,
            stream: false,
            external_reference: None,
            stdio: None,
            sse: None,
            http: None,
        };
        let outcome = self.svc.call_sse(&sse_target, &request).await;
        Ok(Some(match outcome {
            Ok(res) => {
                let passed = res.structured_content.is_some() || !res.content.is_empty();
                CaseResult {
                    name: "call_help_sse".into(),
                    passed,
                    duration_ms: timer.elapsed().as_millis() as u64,
                    detail: self.snapshot(&res),
                }
            }
            Err(err) => CaseResult {
                name: "call_help_sse".into(),
                passed: false,
                duration_ms: timer.elapsed().as_millis() as u64,
                detail: Some(json!({"error": err.to_string()})),
            },
        }))
    }

    async fn call_http_case(&self, target: &ComplianceTarget) -> Result<Option<CaseResult>> {
        let Some(url) = target.http_url.clone() else {
            return Ok(None);
        };
        let timer = Instant::now();
        let http_target = HttpTarget {
            url: url.clone(),
            headers: target.http_headers.clone(),
            auth_token: target.http_auth_token.clone(),
            handshake_timeout_ms: Some(15_000),
        };
        let request = CallRequest {
            tool_name: "help".into(),
            arguments_json: json!({}),
            idempotency_key: None,
            stream: false,
            external_reference: None,
            stdio: None,
            sse: None,
            http: None,
        };
        let outcome = self.svc.call_http(&http_target, &request).await;
        Ok(Some(match outcome {
            Ok(res) => {
                let passed = res.structured_content.is_some() || !res.content.is_empty();
                CaseResult {
                    name: "call_help_http".into(),
                    passed,
                    duration_ms: timer.elapsed().as_millis() as u64,
                    detail: self.snapshot(&res),
                }
            }
            Err(err) => CaseResult {
                name: "call_help_http".into(),
                passed: false,
                duration_ms: timer.elapsed().as_millis() as u64,
                detail: Some(json!({"error": err.to_string()})),
            },
        }))
    }

    async fn call_stdio_stream_case(
        &self,
        target: &ComplianceTarget,
    ) -> Result<Option<CaseResult>> {
        let Some(command) = target.command.as_ref() else {
            return Ok(None);
        };
        let timer = Instant::now();
        let request = CallRequest {
            tool_name: "help".into(),
            arguments_json: json!({}),
            idempotency_key: None,
            stream: true,
            external_reference: None,
            stdio: None,
            sse: None,
            http: None,
        };
        let outcome = self
            .svc
            .call_stdio(
                command.clone(),
                target.args.clone(),
                target.env.clone(),
                target.cwd.clone(),
                &request,
            )
            .await;
        match outcome {
            Ok(res) => {
                let payload = res.structured_content.as_ref().cloned().unwrap_or_default();
                let mode = payload
                    .get("mode")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default();
                let events = payload
                    .get("events")
                    .and_then(|v| v.as_array())
                    .cloned()
                    .unwrap_or_default();
                let has_final = events.iter().any(|event| {
                    event
                        .get("event")
                        .and_then(|value| value.as_str())
                        .map(|kind| kind == "final" || kind == "error")
                        .unwrap_or(false)
                });
                let passed = mode == "stream" && !events.is_empty() && has_final;
                Ok(Some(CaseResult {
                    name: "call_help_stream".into(),
                    passed,
                    duration_ms: timer.elapsed().as_millis() as u64,
                    detail: Some(json!({
                        "mode": mode,
                        "events": events,
                        "snapshot": self.snapshot(&res)
                    })),
                }))
            }
            Err(err) => Ok(Some(CaseResult {
                name: "call_help_stream".into(),
                passed: false,
                duration_ms: timer.elapsed().as_millis() as u64,
                detail: Some(json!({"error": err.to_string()})),
            })),
        }
    }

    async fn probe_sse_case(&self, target: &ComplianceTarget) -> Result<Option<CaseResult>> {
        let Some(url) = target.sse_url.clone() else {
            return Ok(None);
        };
        let timer = Instant::now();
        let req = ProbeRequest {
            transport: Some(TargetTransportKind::Sse),
            command: None,
            args: None,
            env: None,
            cwd: None,
            url: Some(url.clone()),
            headers: None,
            auth_token: None,
            handshake_timeout_ms: Some(15_000),
        };
        let outcome = self.svc.probe(req).await;
        Ok(Some(match outcome {
            Ok(res) => CaseResult {
                name: "probe_sse".into(),
                passed: res.ok,
                duration_ms: timer.elapsed().as_millis() as u64,
                detail: Some(json!({
                    "url": url,
                    "latency_ms": res.latency_ms,
                    "error": res.error,
                })),
            },
            Err(err) => CaseResult {
                name: "probe_sse".into(),
                passed: false,
                duration_ms: timer.elapsed().as_millis() as u64,
                detail: Some(json!({"error": err.to_string()})),
            },
        }))
    }

    async fn probe_http_case(&self, target: &ComplianceTarget) -> Result<Option<CaseResult>> {
        let Some(url) = target.http_url.clone() else {
            return Ok(None);
        };
        let timer = Instant::now();
        let mut req = ProbeRequest::default();
        req.transport = Some(TargetTransportKind::Http);
        req.url = Some(url.clone());
        req.headers = target.http_headers.clone();
        req.auth_token = target.http_auth_token.clone();
        req.handshake_timeout_ms = Some(15_000);
        let outcome = self.svc.probe(req).await;
        Ok(Some(match outcome {
            Ok(res) => CaseResult {
                name: "probe_http".into(),
                passed: res.ok,
                duration_ms: timer.elapsed().as_millis() as u64,
                detail: Some(json!({
                    "url": url,
                    "latency_ms": res.latency_ms,
                    "error": res.error,
                })),
            },
            Err(err) => CaseResult {
                name: "probe_http".into(),
                passed: false,
                duration_ms: timer.elapsed().as_millis() as u64,
                detail: Some(json!({"error": err.to_string()})),
            },
        }))
    }

    async fn missing_command_case(&self) -> Result<CaseResult> {
        let timer = Instant::now();
        let req = ProbeRequest {
            transport: Some(TargetTransportKind::Stdio),
            command: None,
            args: None,
            env: None,
            cwd: None,
            url: None,
            headers: None,
            auth_token: None,
            handshake_timeout_ms: Some(1000),
        };
        let probe = self.svc.probe(req).await;
        let (passed, detail) = match probe {
            Ok(res) => (!res.ok, json!({"expected_error": true, "response": res})),
            Err(err) => (true, json!({"error": err.to_string()})),
        };
        Ok(CaseResult {
            name: "negative_missing_command".into(),
            passed,
            duration_ms: timer.elapsed().as_millis() as u64,
            detail: Some(detail),
        })
    }

    fn snapshot(&self, res: &rmcp::model::CallToolResult) -> Option<Value> {
        serde_json::to_value(res).ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn report_pass_rate() {
        let report = ComplianceReport {
            started_at: OffsetDateTime::now_utc().to_string(),
            finished_at: OffsetDateTime::now_utc().to_string(),
            pass_rate: 0.96,
            cases: vec![CaseResult {
                name: "sample".into(),
                passed: true,
                duration_ms: 10,
                detail: None,
            }],
        };
        assert!(report.passed());
        assert!(report.to_markdown().contains("Pass rate"));
    }
}
