# Real-World MCP Compliance Example

This walkthrough captures a full end-to-end run of the MCP MultiTool compliance suite against the bundled `mock_mcp_server`, exercising SSE and HTTP transports. The generated reports live under `docs/examples/real-world/` and hit a 100 % pass rate (threshold ≥ 95 %).

## Prerequisites

- Rust toolchain (`cargo`) on PATH.
- Repository cloned locally with the default toolchain components installed.

## Steps

1. **Build release binaries** so the compliance runner and mock server are ready:
   ```bash
   cargo build --release -p mock_mcp_server -p mcp_multi_tool
   ```

2. **Start the mock server** in a dedicated shell with stdio disabled (SSE/HTTP stay active):
   ```bash
   MOCK_ENABLE_STDIO=0 target/release/mock_mcp_server
   ```

   The server binds `http://127.0.0.1:9100/sse` and `http://127.0.0.1:9101/mcp`.

3. **Run the compliance suite** from another shell, capturing JSON and Markdown artefacts:
   ```bash
   target/release/compliance \
     --sse-url http://127.0.0.1:9100/sse \
     --http-url http://127.0.0.1:9101/mcp \
     --http-header accept=application/json \
     --output-json docs/examples/real-world/report.json \
     --output-md docs/examples/real-world/report.md
   ```

   The command prints the summary to stdout and exits with code `0` when the pass rate is ≥ 95 %.

4. **Shut down the mock server** with `Ctrl+C` once the run finishes.

## Artefacts

- `docs/examples/real-world/report.json` — machine-readable compliance report with per-case detail.
- `docs/examples/real-world/report.md` — Markdown table ready for status pages or changelogs.

These files are committed to the repository so downstream agents can inspect a deterministic real-world run without recomputing it.
