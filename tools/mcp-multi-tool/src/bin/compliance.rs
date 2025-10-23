use std::{collections::BTreeMap, fs, path::PathBuf};

use anyhow::{Context, Result, anyhow};
use clap::Parser;
use mcp_multi_tool::app::compliance::{ComplianceSuite, ComplianceTarget};

#[derive(Parser, Debug)]
#[command(
    author,
    version,
    about = "Run MCP MultiTool compliance checks against a target MCP server."
)]
struct Args {
    /// Command to launch the target stdio MCP server
    #[arg(long)]
    command: Option<String>,

    /// Arguments passed to the target command (repeat flag for multiple entries)
    #[arg(long)]
    args: Vec<String>,

    /// Environment variables KEY=VALUE (repeat flag)
    #[arg(long, value_parser = parse_env)]
    env: Vec<(String, String)>,

    /// Working directory for the target process
    #[arg(long)]
    cwd: Option<PathBuf>,

    /// Optional SSE endpoint for probe validation
    #[arg(long)]
    sse_url: Option<String>,

    /// Optional HTTP endpoint for probe validation
    #[arg(long)]
    http_url: Option<String>,

    /// HTTP headers KEY=VALUE (repeat flag)
    #[arg(long, value_parser = parse_env)]
    http_header: Vec<(String, String)>,

    /// HTTP auth token (Bearer)
    #[arg(long)]
    http_auth_token: Option<String>,

    /// Path to write the JSON report (optional)
    #[arg(long)]
    output_json: Option<PathBuf>,

    /// Path to write the Markdown report (optional)
    #[arg(long)]
    output_md: Option<PathBuf>,
}

fn parse_env(raw: &str) -> Result<(String, String)> {
    let (key, value) = raw
        .split_once('=')
        .ok_or_else(|| anyhow!("env entry must be KEY=VALUE"))?;
    Ok((key.to_string(), value.to_string()))
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    let mut env_map: BTreeMap<String, String> = BTreeMap::new();
    for (k, v) in args.env {
        env_map.insert(k, v);
    }

    let mut http_headers = BTreeMap::new();
    for (k, v) in args.http_header {
        http_headers.insert(k, v);
    }

    let target = ComplianceTarget {
        command: args.command,
        args: args.args,
        env: if env_map.is_empty() {
            None
        } else {
            Some(env_map)
        },
        cwd: args.cwd.as_ref().map(|p| p.to_string_lossy().to_string()),
        sse_url: args.sse_url,
        http_url: args.http_url,
        http_headers: if http_headers.is_empty() {
            None
        } else {
            Some(http_headers)
        },
        http_auth_token: args.http_auth_token,
    };

    let suite = ComplianceSuite::new();
    let report = suite.run(target).await.context("run compliance suite")?;

    let json_report = serde_json::to_string_pretty(&report)?;
    println!("{}", json_report);

    if let Some(path) = args.output_json {
        fs::write(&path, &json_report)
            .with_context(|| format!("write json report to {}", path.display()))?;
    }
    if let Some(path) = args.output_md {
        fs::write(&path, report.to_markdown())
            .with_context(|| format!("write markdown report to {}", path.display()))?;
    }

    if !report.passed() {
        eprintln!(
            "compliance pass rate below 95% (actual {:.2}%)",
            report.pass_rate * 100.0
        );
        std::process::exit(1);
    }

    Ok(())
}
