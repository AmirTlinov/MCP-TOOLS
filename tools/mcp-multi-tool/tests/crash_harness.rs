use std::time::Duration;

use anyhow::Result;
use assert_cmd::cargo::cargo_bin;
use rmcp::{
    ServiceExt,
    model::CallToolRequestParam,
    transport::child_process::{ConfigureCommandExt, TokioChildProcess},
};
use tempfile::tempdir;
use tokio::process::Command;

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn inspector_outbox_falls_back_to_dlq_on_write_failure() -> Result<()> {
    let bin = cargo_bin("mcp-multi-tool");
    let tmp = tempdir()?;
    let primary_dir = tmp.path().join("primary_dir");
    std::fs::create_dir_all(&primary_dir)?;
    let dlq_path = tmp.path().join("dlq.jsonl");

    let service = ()
        .serve(TokioChildProcess::new(Command::new(&bin).configure(
            |cmd| {
                cmd.env("OUTBOX_PATH", primary_dir.to_string_lossy().to_string());
                cmd.env("OUTBOX_DLQ_PATH", dlq_path.to_string_lossy().to_string());
                cmd.env("ERROR_BUDGET_ENABLED", "false");
            },
        ))?)
        .await?;

    let req = serde_json::json!({
        "tool_name": "help",
        "arguments_json": {},
        "stream": false,
        "stdio": {
            "command": "/bin/sh",
            "args": ["-c", "exit 1"],
        }
    });

    let response = service
        .call_tool(CallToolRequestParam {
            name: "inspector_call".into(),
            arguments: Some(req.as_object().cloned().unwrap()),
        })
        .await?;
    assert!(response.is_error.unwrap_or(false));

    tokio::time::sleep(Duration::from_millis(100)).await;
    let dlq_data = tokio::fs::read_to_string(&dlq_path).await?;
    assert!(dlq_data.contains("error"));
    let first_line = dlq_data
        .lines()
        .find(|line| !line.trim().is_empty())
        .unwrap();
    let entry: serde_json::Value = serde_json::from_str(first_line)?;
    assert_eq!(
        entry.get("tool_name").and_then(|v| v.as_str()),
        Some("help")
    );
    assert!(entry.get("error").is_some());

    Ok(())
}
