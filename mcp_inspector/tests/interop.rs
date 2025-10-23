use anyhow::Result;
use assert_cmd::cargo::cargo_bin;
use rmcp::{
    ServiceExt,
    model::CallToolRequestParam,
    transport::child_process::{ConfigureCommandExt, TokioChildProcess},
};
use tokio::process::Command;

#[tokio::test]
async fn list_tools_and_help() -> Result<()> {
    let bin = cargo_bin("mcp_inspector");
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
    Ok(())
}
