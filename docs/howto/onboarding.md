# MCP MultiTool Onboarding Guide

## 1. Prerequisites
- Rust toolchain (rustup ≥ 1.81) with `cargo`, `cargo fmt`, `cargo clippy`.
- `just` (optional) for shortcut recipes.
- Access to the repository and ability to open the MCP port from your automation host.

## 2. Build Artifacts
```bash
cargo build --release -p mcp_multi_tool
cargo build --release -p mock_mcp_server # optional: local target for smoke tests
```
Release binaries land in `target/release/`.

## 3. Baseline Configuration
`config/default.toml` ships with production-safe defaults:
- `metrics_addr` — Prometheus listener (set TLS + auth for production).
- `outbox_*` — file paths for durable run events (use sqlite via `OUTBOX_DB_PATH` for stronger guarantees).
- `idempotency_conflict_policy` — defaults to `conflict_409`; aliases (`conflict_409`, `conflict`, `conflict409`) are accepted.

The server reads overlays in order:
1. `config/default.toml`
2. `config/<profile>.toml` controlled by `APP_CONFIG_PROFILE`
3. `config/local.toml`
4. Environment variables (`METRICS_ADDR`, `OUTBOX_PATH`, `IDEMPOTENCY_CONFLICT_POLICY`, etc.)

Validate your bundle before shipping:
```bash
cargo test --lib infra::config::tests::default_config_parses
```

## 4. Error-Budget Freeze
The inspector halts `inspector_call` whenever success rate falls below the configured SLO window.

Key environment toggles:
```bash
export ERROR_BUDGET_ENABLED=true
export ERROR_BUDGET_SUCCESS_THRESHOLD=0.95   # minimum success ratio
export ERROR_BUDGET_SAMPLE_WINDOW_SECS=120   # rolling window in seconds
export ERROR_BUDGET_MIN_REQUESTS=20          # minimum samples before evaluation
export ERROR_BUDGET_FREEZE_SECS=300          # freeze duration when breached
```

During a freeze responses include:
```json
{
  "error": "error budget exhausted",
  "code": "ERROR_BUDGET_EXHAUSTED",
  "frozen_until": "2025-10-23T20:32:30Z",
  "success_rate": 0.6,
  "sample_size": 20
}
```
Monitor the Prometheus gauge `error_budget_frozen` (1 = freeze active).

## 5. Registering the MCP Server (Codex example)
Add to `~/.codex/config.toml`:
```toml
[mcp_servers.mcp_multi_tool]
command = "/path/to/repo/target/release/mcp-multi-tool"
startup_timeout_sec = 30.0

[mcp_servers.mcp_multi_tool.env]
ERROR_BUDGET_ENABLED = "true"
ERROR_BUDGET_SUCCESS_THRESHOLD = "0.95"
ERROR_BUDGET_SAMPLE_WINDOW_SECS = "120"
ERROR_BUDGET_MIN_REQUESTS = "20"
ERROR_BUDGET_FREEZE_SECS = "300"
```

Restart Codex CLI (or the relevant orchestrator) so the server advertises its tools: `help`, `inspector_probe`, `inspector_list_tools`, `inspector_describe`, `inspector_call`.

## 6. Smoke Test Against the Mock Server
```bash
target/release/mock_mcp_server &
MOCK_PID=$!

target/release/mcp_inspector \
  --command "target/release/mcp-multi-tool" \
  inspector_list_tools

kill $MOCK_PID
```
You should see the five inspector tools and the mock server logs confirming successful calls.

## 7. Production Checklist
- Set `METRICS_AUTH_TOKEN`, `METRICS_TLS_CERT_PATH`, `METRICS_TLS_KEY_PATH`.
- Promote sqlite outbox via `OUTBOX_DB_PATH` for durable persistence.
- Pin `ERROR_BUDGET_*` to match your incident policy; default freeze is 5 minutes with 95% minimum success.
- Monitor Prometheus metrics: `inspector_inflight`, `outbox_backlog`, `error_budget_frozen`, latency histograms.
- Keep `cargo test` and `cargo clippy --all-targets --all-features -D warnings` green before release.
