use anyhow::Result;
use assert_cmd::cargo::cargo_bin;
use rmcp::{
    ServiceExt,
    model::CallToolRequestParam,
    transport::child_process::{ConfigureCommandExt, TokioChildProcess},
};
use serde_json::json;
use tokio::process::Command;

#[tokio::test]
async fn error_budget_freeze_blocks_calls() -> Result<()> {
    let bin = cargo_bin("mcp-multi-tool");
    let service = ()
        .serve(TokioChildProcess::new(Command::new(&bin).configure(
            |cmd| {
                cmd.env("ERROR_BUDGET_ENABLED", "true");
                cmd.env("ERROR_BUDGET_SUCCESS_THRESHOLD", "0.6");
                cmd.env("ERROR_BUDGET_MIN_REQUESTS", "3");
                cmd.env("ERROR_BUDGET_SAMPLE_WINDOW_SECS", "120");
                cmd.env("ERROR_BUDGET_FREEZE_SECS", "60");
            },
        ))?)
        .await?;

    let failing = json!({
        "tool_name": "help",
        "arguments_json": {},
        "stream": false,
        "stdio": {
            "command": "definitely-not-a-binary"
        }
    });
    let failing_map = failing.as_object().cloned().unwrap();

    for _ in 0..3 {
        let result = service
            .call_tool(CallToolRequestParam {
                name: "inspector_call".into(),
                arguments: Some(failing_map.clone()),
            })
            .await?;
        assert!(result.is_error.unwrap_or(false));
    }

    let frozen = service
        .call_tool(CallToolRequestParam {
            name: "inspector_call".into(),
            arguments: Some(failing_map.clone()),
        })
        .await?;
    assert!(frozen.is_error.unwrap_or(false));
    let payload = frozen
        .structured_content
        .expect("freeze structured payload");
    assert_eq!(
        payload
            .get("code")
            .and_then(|value| value.as_str())
            .unwrap_or(""),
        "ERROR_BUDGET_EXHAUSTED"
    );

    Ok(())
}
