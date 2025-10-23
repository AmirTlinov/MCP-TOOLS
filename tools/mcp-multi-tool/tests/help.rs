use anyhow::Result;
use assert_cmd::cargo::cargo_bin;
use rmcp::{
    ServiceExt,
    model::CallToolRequestParam,
    transport::child_process::{ConfigureCommandExt, TokioChildProcess},
};
use serde_json::Value;
use tokio::process::Command;

#[tokio::test]
async fn help_returns_structured_jsonl() -> Result<()> {
    let bin = cargo_bin("mcp-multi-tool");
    let service = ()
        .serve(TokioChildProcess::new(Command::new(&bin).configure(
            |cmd| {
                cmd.env("ERROR_BUDGET_ENABLED", "false");
            },
        ))?)
        .await?;

    let help = service
        .call_tool(CallToolRequestParam {
            name: "help".into(),
            arguments: None,
        })
        .await?;

    let payload = help.structured_content.expect("help structured content");
    assert_eq!(
        payload.get("format").and_then(|v| v.as_str()),
        Some("jsonl")
    );
    let lines = payload
        .get("lines")
        .and_then(|v| v.as_array())
        .cloned()
        .expect("jsonl lines");
    assert!(lines.len() >= 6, "expected at least six jsonl entries");

    for line in &lines {
        let line_str = line.as_str().expect("line string");
        let entry: Value = serde_json::from_str(line_str)?;
        assert!(entry.get("section").is_some());
    }

    Ok(())
}
