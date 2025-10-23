# TODO.md — Deterministic Implementation Checklist (Flagship+++)

Statuses: [ ] todo · [~] wip · [x] done · [b] blocked.
Format: `<ID> — <concise action> — DoD`. Every item maps to a T-ID.

## 0. Environment Prep
- [x] ENV: define variables (`ALLOW_INSECURE_METRICS_DEV`, tokens) — DoD: `.env.example` committed.
- [x] Toolchain: `rustup 1.81+`, `cargo-deny`, `cargo-audit`, `just` — DoD: `just --list` succeeds.

## 1. Kickoff (M-0001)
- [x] T-0001 — Initialize workspace and crate — DoD: `cargo build` passes locally.
- [x] T-0002 — Pin `rmcp = 0.8.1` and required features — DoD: `cargo tree` stable; `Cargo.lock` updated.
- [x] T-0003 — Domain `InspectionRun` (STATE_MACHINE) — DoD: state transition tests green.
- [ ] T-0004 — Config via ENV/`config/` — DoD: config unit tests cover precedence.
- [x] T-0005 — Stdio MCP server handshake — DoD: integration handshake test green.
- [x] T-0006 — Tool registry and manifest — DoD: `list_tools` returns registered tools.

## 2. Target MCP Clients (WS-002)
- [x] T-0007 — Stdio client (spawn + pipes) — DoD: E2E against mock passes.
- [x] T-0008 — SSE client (subscription/reconnect) — DoD: reconnect <5s.
- [x] T-0009 — Streamable HTTP client — DoD: chunk assembly without leaks.
- [x] T-0010 — Probe (connect/version/latency) — DoD: returns version/latency/transport.

## 3. Inspector Core Operations (WS-003)
- [ ] T-0011 — `inspector.list_tools` — DoD: matches mock baseline.
- [ ] T-0012 — `inspector.describe` (+JSON Schema) — DoD: 100% validated descriptions.
- [ ] T-0013 — `inspector.call` (with trace) — DoD: E2E matches baseline.
- [ ] T-0015 — Streaming onChunk/onFinal — DoD: mixed streaming test green.
- [x] T-0016 — Compliance suite — DoD: deterministic JSON/MD report.
- [ ] T-0030 — Real-world E2E example — DoD: ≥90% pass with report attached.

## 4. Reliability & Idempotency (WS-004)
- [ ] T-0014 — Idempotency (CLAIM + key) — DoD: property tests stable 3×100 runs.
- [~] T-0031 — Transactional Outbox — DoD: zero loss during crash tests.
- [ ] T-0032 — Reaper TTL = 60s — DoD: stuck→failed, event + metric emitted.
- [ ] T-0033 — Compensation `external_ref_unique` — DoD: compensation scenarios green.

## 5. Observability & SLO (WS-005)
- [ ] T-0017 — Latency metrics p50/p95/p99 — DoD: Prometheus scrape works.
- [ ] T-0018 — `/metrics` with Auth + TLS (+dev flag) — DoD: denied without auth; dev override works.
- [ ] T-0019 — Logs/trace with correlation IDs — DoD: structured logging validated.
- [ ] T-0035 — Error budget freeze — DoD: gate trips in CI when SLO breached.

## 6. Contracts & Docs (WS-006)
- [x] T-0020 — Event JSON Schemas — DoD: 100% validation coverage.
- [x] T-0021 — Public contracts handbook — DoD: walkthrough ≤15 minutes.
- [ ] T-0027 — How-To docs — DoD: onboarding doc published.

## 7. Quality Gates (WS-007)
- [x] T-0022 — CI coverage gate 85% — DoD: pipeline fails under threshold.
- [x] T-0023 — Mock MCP server — DoD: E2E harness ready.
- [ ] T-0024 — Property tests for idempotency — DoD: 3×100 seeds stable.
- [ ] T-0025 — Race tests — DoD: lock wait p99 ≤50ms.
- [ ] T-0026 — Crash/edge harness — DoD: zero panics across runs.
- [ ] T-0036 — ACL/dependency rules — DoD: CI fails on violations.
- [x] T-0037 — Lint/security audits — DoD: zero critical findings.
- [ ] T-0038 — Architecture tests — DoD: zero dependency cycles.

## 8. Release & Ops (WS-008)
- [x] T-0028 — Release artifacts (Linux/macOS/Windows) — DoD: downloadable binaries run.
- [ ] T-0029 — Security baseline — DoD: no critical vulnerabilities.
- [ ] T-0034 — Canary/Rollback toggle — DoD: feature flag validated.
