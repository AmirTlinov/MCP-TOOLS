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
async fn list_tools_and_help() -> Result<()> {
    let bin = cargo_bin("mcp-multi-tool");
    println!("using binary: {}", bin.display());
    let service = ()
        .serve(TokioChildProcess::new(Command::new(&bin).configure(|c| {
            c.env("RUST_LOG", "info");
        }))?)
        .await?;
    println!("connected");
    let tools = service.list_tools(Default::default()).await?.tools;
    println!("tools response");
    assert!(tools.iter().any(|t| t.name.as_ref() == "help"));
    let help = service
        .call_tool(CallToolRequestParam {
            name: "help".into(),
            arguments: None,
        })
        .await?;
    assert!(help.structured_content.is_some());

    let mock = cargo_bin("mock_mcp_server");
    let list_args = json!({
        "transport": "stdio",
        "command": mock.display().to_string(),
        "args": [],
        "handshake_timeout_ms": 5000
    });
    let list_resp = service
        .call_tool(CallToolRequestParam {
            name: "inspector_list_tools".into(),
            arguments: Some(list_args.as_object().cloned().unwrap()),
        })
        .await?;
    let payload = list_resp
        .structured_content
        .expect("list_tools structured payload");
    let listed = payload
        .get("tools")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    assert!(listed.iter().any(|tool| {
        tool.get("name")
            .and_then(|n| n.as_str())
            .map(|name| name == "help")
            .unwrap_or(false)
    }));

    let describe_args = json!({
        "tool_name": "help",
        "transport": "stdio",
        "command": mock.display().to_string(),
        "args": []
    });
    let describe_resp = service
        .call_tool(CallToolRequestParam {
            name: "inspector_describe".into(),
            arguments: Some(describe_args.as_object().cloned().unwrap()),
        })
        .await?;
    let describe_payload = describe_resp
        .structured_content
        .expect("describe structured payload");
    let described = describe_payload
        .get("tool")
        .and_then(|tool| tool.get("name"))
        .and_then(|name| name.as_str())
        .unwrap_or_default();
    assert_eq!(described, "help");
    Ok(())
}
