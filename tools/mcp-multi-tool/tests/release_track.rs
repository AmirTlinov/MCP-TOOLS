use anyhow::Result;
use assert_cmd::cargo::cargo_bin;
use rmcp::{
    ServiceExt,
    model::CallToolRequestParam,
    transport::child_process::{ConfigureCommandExt, TokioChildProcess},
};
use serde_json::{Map, Value};
use tokio::process::Command;

#[tokio::test]
async fn rollback_disables_inspector_tools() -> Result<()> {
    let bin = cargo_bin("mcp-multi-tool");
    let service = ()
        .serve(TokioChildProcess::new(Command::new(&bin).configure(
            |cmd| {
                cmd.env("RELEASE_TRACK", "rollback");
            },
        ))?)
        .await?;

    let listed = service.list_tools(Default::default()).await?.tools;
    assert_eq!(listed.len(), 1, "rollback should expose only help tool");
    assert_eq!(listed[0].name.as_ref(), "help");

    let mut args = Map::new();
    args.insert("tool_name".into(), Value::String("help".into()));
    args.insert("arguments_json".into(), Value::Object(Map::new()));
    args.insert("stream".into(), Value::Bool(false));

    let response = service
        .call_tool(CallToolRequestParam {
            name: "inspector_call".into(),
            arguments: Some(args),
        })
        .await?;

    assert!(response.is_error.unwrap_or(false));
    let payload = response
        .structured_content
        .expect("structured error payload expected");
    assert_eq!(
        payload.get("code").and_then(Value::as_str),
        Some("RELEASE_TRACK_ROLLBACK")
    );

    Ok(())
}
