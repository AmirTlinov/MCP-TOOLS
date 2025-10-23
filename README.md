# MCP TOOLS Monorepo

MCP TOOLS is the shared home for every Model Context Protocol (MCP) solution we ship. The first delivery is **MCP MultiTool**, a Rust stdio server + client that lets AI agents inspect and stress-test third-party MCP services with minimal friction.

## Repository Layout

| Path | Purpose |
| --- | --- |
| `tools/mcp-multi-tool/` | Primary binary crate. Hosts the MCP MultiTool stdio server and inspector. |
| `data/` | Lightweight artifacts (for example, sample outbox/dlq payloads). |
| `docs/` | Architecture notes, public contracts, JSON Schemas (expands as features land). |
| `config/` | Reserved for configuration bundles (added together with upcoming services). |

## Quickstart

```bash
git clone git@github.com:iMAGRAY/MCP-TOOLS.git
cd MCP-TOOLS
cargo run -p mcp_multi_tool
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
```

Check `CONTRIBUTING.md` for the full checklist. New tools live under `tools/<tool-name>` and must be registered in the workspace manifest.

## License

This project is released under [The Unlicense](LICENSE). You can use it for any purpose with no restrictions.
