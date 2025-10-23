use anyhow::Result;
use rmcp::{
    ServiceExt,
    model::CallToolRequestParam,
    transport::child_process::{ConfigureCommandExt, TokioChildProcess},
};
use tokio::process::Command;

#[tokio::main]
async fn main() -> Result<()> {
    let target_dir = std::env::var("CARGO_TARGET_DIR").unwrap_or_else(|_| "target".into());
    let profile = if cfg!(debug_assertions) {
        "debug"
    } else {
        "release"
    };
    let server_bin = format!("{}/{}/mcp-multi-tool", target_dir, profile);

    let service = ()
        .serve(TokioChildProcess::new(
            Command::new(&server_bin).configure(|c| {
                c.env("RUST_LOG", "trace");
            }),
        )?)
        .await?;

    // List tools
    let tools = service.list_tools(Default::default()).await?.tools;
    println!(
        "tools_count={} names={:?}",
        tools.len(),
        tools.iter().map(|t| t.name.to_string()).collect::<Vec<_>>()
    );

    // Call help
    let help = service
        .call_tool(CallToolRequestParam {
            name: "help".into(),
            arguments: None,
        })
        .await?;
    if let Some(json) = help.structured_content {
        println!(
            "help_structured_keys={:?}",
            json.as_object()
                .map(|m| m.keys().cloned().collect::<Vec<_>>())
                .unwrap_or_default()
        );
        println!(
            "help_tldr={}",
            json.get("tldr").map(|v| v.to_string()).unwrap_or_default()
        );
    } else {
        println!("help_no_structured_content");
    }

    Ok(())
}
