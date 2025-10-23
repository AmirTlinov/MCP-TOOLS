use anyhow::Result;
use assert_cmd::cargo::cargo_bin;
use serde_json::Value;
use std::process::Command;

mod harness;
use harness::MockMcpCommand;

#[test]
fn compliance_self_check_passes() -> Result<()> {
    let target_path = MockMcpCommand::stdio_path();
    let mut cmd = Command::new(cargo_bin("compliance"));
    cmd.arg("--command").arg(target_path);
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
