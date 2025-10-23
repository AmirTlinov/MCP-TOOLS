# Mission Overview

Our mission is to build a Rust-based MCP stdio server that becomes the fastest way to craft and validate complex MCP servers.

## Focus Areas
1. Ship the **MCP MultiTool** so any AI agent can rapidly probe, describe, and exercise arbitrary MCP servers.
2. Maintain a single binary distribution: zero CLI flags, configuration only via `config/` and environment variables so the binary plugs into any AI CLI (Codex, Claude Code, Gemini, etc.).
3. Track upstream standards aggressively. Pin runtime on `rmcp = 0.8.1` and MCP protocol revision 2025-06-18 until the next validated release.
