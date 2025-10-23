use std::{
    net::TcpListener,
    path::Path,
    process::{Command, Stdio},
    thread,
    time::Duration,
};

use anyhow::Result;
use assert_cmd::cargo::cargo_bin;
use serde_json::Value;

mod harness;
use harness::{mcp_multi_tool_path, mock_server_path};

#[test]
fn compliance_self_check_passes() -> Result<()> {
    let mut cmd = Command::new(cargo_bin("compliance"));
    cmd.arg("--command").arg(mcp_multi_tool_path());
    let output = cmd.output()?;
    assert!(
        output.status.success(),
        "compliance binary exited with failure: {:?}",
        output
    );
    let report: Value = serde_json::from_slice(&output.stdout)?;
    let pass_rate = report
        .get("pass_rate")
        .and_then(Value::as_f64)
        .unwrap_or_default();
    assert!(
        pass_rate >= 0.95,
        "pass rate below threshold: {}",
        pass_rate
    );
    Ok(())
}

fn reserve_port() -> Result<u16> {
    let listener = TcpListener::bind("127.0.0.1:0")?;
    let port = listener.local_addr()?.port();
    drop(listener);
    Ok(port)
}

#[test]
fn compliance_with_mock_transports() -> Result<()> {
    let status = Command::new("cargo")
        .args(["build", "-p", "mock_mcp_server"])
        .status()?;
    assert!(status.success(), "failed to build mock_mcp_server binary");

    let sse_port = reserve_port()?;
    let http_port = reserve_port()?;

    let server_path = mock_server_path();
    assert!(Path::new(&server_path).exists());
    let mut server = Command::new(&server_path)
        .env("MOCK_ENABLE_STDIO", "0")
        .env("MOCK_SSE_ADDR", format!("127.0.0.1:{sse_port}"))
        .env("MOCK_HTTP_ADDR", format!("127.0.0.1:{http_port}"))
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()?;

    thread::sleep(Duration::from_millis(200));

    let mut cmd = Command::new(cargo_bin("compliance"));
    cmd.arg("--sse-url")
        .arg(format!("http://127.0.0.1:{sse_port}/sse"));
    cmd.arg("--http-url")
        .arg(format!("http://127.0.0.1:{http_port}/mcp"));
    cmd.arg("--http-header").arg("accept=application/json");

    let output = cmd.output()?;
    let _ = server.kill();
    let _ = server.wait();
    assert!(
        output.status.success(),
        "compliance with mock server failed: {:?}",
        output
    );
    let report: Value = serde_json::from_slice(&output.stdout)?;
    let cases = report
        .get("cases")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let mut has_http = false;
    let mut has_sse = false;
    let mut has_describe_http = false;
    let mut has_describe_sse = false;
    for case in cases {
        if let Some(name) = case.get("name").and_then(Value::as_str) {
            if name == "call_help_http" {
                has_http = true;
            } else if name == "call_help_sse" {
                has_sse = true;
            } else if name == "describe_help_http" {
                has_describe_http = true;
            } else if name == "describe_help_sse" {
                has_describe_sse = true;
            }
        }
    }
    assert!(has_sse, "missing SSE call case");
    assert!(has_http, "missing HTTP call case");
    assert!(has_describe_sse, "missing SSE describe case");
    assert!(has_describe_http, "missing HTTP describe case");
    Ok(())
}
