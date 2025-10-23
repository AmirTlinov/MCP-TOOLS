use anyhow::Result;
use assert_cmd::cargo::cargo_bin;
use rmcp::{
    ServiceExt,
    model::CallToolRequestParam,
    transport::child_process::{ConfigureCommandExt, TokioChildProcess},
};
use serde_json::{Value, json};
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
    let help_payload = help.structured_content.expect("help structured content");
    assert_eq!(
        help_payload
            .get("format")
            .and_then(|v| v.as_str())
            .unwrap_or_default(),
        "jsonl"
    );
    let lines = help_payload
        .get("lines")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    assert!(!lines.is_empty());
    let summary: Value =
        serde_json::from_str(lines[0].as_str().expect("jsonl summary line string"))?;
    assert_eq!(
        summary.get("section").and_then(Value::as_str),
        Some("summary")
    );

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

    let stream_args = json!({
        "tool_name": "stream",
        "arguments_json": {},
        "stream": true,
        "stdio": {
            "command": mock.display().to_string(),
            "args": []
        }
    });
    let stream_resp = service
        .call_tool(CallToolRequestParam {
            name: "inspector_call".into(),
            arguments: Some(stream_args.as_object().cloned().unwrap()),
        })
        .await?;
    let stream_trace = stream_resp
        .meta
        .as_ref()
        .and_then(|meta| meta.get("trace"))
        .expect("stream trace meta");
    assert!(
        stream_trace
            .get("stream_enabled")
            .and_then(|value| value.as_bool())
            .unwrap_or(false)
    );
    let stream_payload = stream_resp
        .structured_content
        .expect("stream structured payload");
    assert_eq!(
        stream_payload
            .get("mode")
            .and_then(|v| v.as_str())
            .unwrap_or_default(),
        "stream"
    );
    let events = stream_payload
        .get("events")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    assert!(events.iter().any(|event| {
        event
            .get("event")
            .and_then(|value| value.as_str())
            .map(|kind| kind == "final" || kind == "error")
            .unwrap_or(false)
    }));
    let trace_events = stream_trace
        .get("stream_events")
        .and_then(|value| value.as_array())
        .cloned()
        .unwrap_or_default();
    assert!(!trace_events.is_empty());
    Ok(())
}
