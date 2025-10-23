# Security Baseline — MCP MultiTool (GA)

_Last refreshed: 2025-10-23_

## Tooling
- `cargo deny check advisories` — primary source of RustSec advisories.
- `cargo deny check licenses` / `cargo deny check bans` (run as needed) — verifies license policy and duplicate crates.
- Dev workflow: `just security` (see `justfile`) runs the advisories check.

## Current Status
- ✅ `cargo deny check advisories` (2025-10-23) — no actionable vulnerabilities.
- ⚠️ Accepted advisory: `RUSTSEC-2024-0436` (`paste` crate unmaintained). Rationale: `rmcp = 0.8.1` (protocol pin) depends on `paste 1.0.15`; no maintained fork released. Tracking upstream issue; ignore entry expires 2026-01-01.
- ✅ `cargo deny check bans` / `licenses` — zero violations.

## Operational Guidance
1. Install tooling: `cargo install cargo-deny`.
2. Update advisory DB: `cargo deny fetch`.
3. Run security sweep: `just security` (delegates to `cargo deny check advisories`).
4. For release tags, capture the command output and store it alongside the build artifacts.
5. Re-evaluate ignored advisories every 30 days or on `rmcp` upgrades.

## Playbook
- **New advisory introduced**: fail CI, assess upgrade path; if blocked, document justification + expiry in `deny.toml`.
- **License violation**: prefer dependency swap; escalate to legal before shipping.
- **Tooling failure**: rerun with `--no-warn --log-level trace` and open SRE ticket if infrastructure-related.

## Change Log
- 2025-10-23 — Baseline captured; Prometheus dependency upgraded to 0.14.0 (drops vulnerable `protobuf`). Ignore pinned for `paste` advisory.
