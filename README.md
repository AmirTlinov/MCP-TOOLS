# MCP TOOLS Monorepo

MCP TOOLS is the shared home for every Model Context Protocol (MCP) solution we ship. The first delivery is **MCP MultiTool**, a Rust stdio server + client that lets AI agents inspect and stress-test third-party MCP services with minimal friction.

## Repository Layout

| Path | Purpose |
| --- | --- |
| `tools/mcp-multi-tool/` | Primary binary crate. Hosts the MCP MultiTool stdio server and inspector. |
| `data/` | Lightweight artifacts (for example, sample outbox/dlq payloads). |
| `docs/` | Architecture notes, metrics spec, public contracts, JSON Schemas. |
| `config/` | Reserved for configuration bundles (added together with upcoming services). |

## Quickstart

```bash
git clone git@github.com:iMAGRAY/MCP-TOOLS.git
cd MCP-TOOLS
cargo run -p mcp_multi_tool
# optional: load defaults
cp .env.example .env
```

Any MCP-capable agent (Codex CLI, Claude Code, Gemini Code Assist, etc.) can connect to the binary via stdio without additional flags.

## Quality Bar

- Architecture: Modular Monolith with DDD and Ports & Adapters; use CQRS where it buys clarity.
- Contracts: MCP 2025-06-18, `rmcp = 0.8.1`, public JSON Schemas for all emitted events.
- Reliability: idempotency via `idempotency_key`, transactional outbox with 60s reaper TTL.
- Observability: Prometheus `/metrics`, TLS + Auth (dev override `ALLOW_INSECURE_METRICS_DEV=true`).
- Tests: `cargo test` plus property/race tests, coverage ≥85% Lines & Statements for touched code.
- CI: GitHub Actions pipeline runs fmt, clippy, tests, coverage gate.

## MCP MultiTool Highlights

- Rapid attach to target MCP servers (stdio / SSE / streamable HTTP) with full `list_tools`, `describe`, `call`, and streaming coverage.
- Smoketest binary: `cargo run -p mcp_multi_tool --bin smoketest` spins up the server and exercises a happy path.
- Integration test `tests/interop.rs` boots the binary and performs remote calls via rmcp APIs.

## Development Workflow

```bash
# Formatting and linting
cargo fmt
cargo clippy --all-targets --all-features

# Unit + integration tests
cargo test

# Coverage (requires llvm-tools-preview)
cargo llvm-cov --lcov --output-path coverage.lcov --fail-under-lines 85

# Compliance against a target MCP server
just compliance command="$(which mcp-server-binary)"
```

Check `CONTRIBUTING.md` for the full checklist. New tools live under `tools/<tool-name>` and must be registered in the workspace manifest. Additional references: architecture diagram (`docs/architecture/mcp-multi-tool.md`), metrics spec (`docs/metrics.md`), and contract schemas (`docs/contracts/`).

## Releases

Tagged commits (`v*`) trigger `.github/workflows/release.yml`, producing binaries for Linux (`x86_64-unknown-linux-gnu`), macOS (`aarch64-apple-darwin`), and Windows (`x86_64-pc-windows-msvc`). Artifacts ship alongside a checksum file. Use `cargo run --release -p mcp_multi_tool` for local smoke tests before tagging.

## Compliance Suite

`cargo run --release -p mcp_multi_tool --bin compliance -- --command <target>` spawns a target MCP stdio server, runs probe/list/call checks, and emits a JSON report (exit code 1 if pass rate <95%). Combine with `--output-json` / `--output-md` for archival.

## License

This project is released under [The Unlicense](LICENSE). You can use it for any purpose with no restrictions.
