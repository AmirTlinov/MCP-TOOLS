# Plan.md — MCP Stdio Server + MCP MultiTool (Rust, rmcp 0.8.1)

## 1) Executive Summary
- Goal: deliver a single Rust binary (stdio MCP server) with the MCP MultiTool inspector for fast, complete MCP testing (stdio/SSE/HTTP).
- Architecture: Modular Monolith (DDD, Ports & Adapters), public MCP interface without CLI flags; configuration via `config/` and environment variables only.
- MVP: connect to target MCP, list tools, describe schemas, invoke tools (incl. streaming), run compliance suite, expose metrics, publish report.
- Quality: coverage ≥85% (Statements/Lines), SLO p99 ≤200 ms, exactly-once ratio 1.00±0.01, zero dependency cycles.
- Reliability: idempotency via `idempotency_key`, SingleEffect (CLAIM|OUTBOX), stuck reaper TTL 60 s.
- Timeline: M-0001 Kickoff → M-0002 MVP Alpha (2 weeks) → M-0003 Beta (3.5 weeks) → M-0004 GA (5 weeks total).
- Deliverables: binary, public contracts & JSON Schemas, instructions for Codex/Claude/Gemini integrations.

## 2) Goals & Success Metrics
- G1: Ship a single binary without CLI flags by M-0004; metric — downloadable artifacts for Linux/macOS/Windows.
- G2: Inspector covers ≥95% of MCP operations (connect/list/describe/call/stream); metric — compliance suite pass rate ≥95%.
- G3: Code quality; metric — Statements/Lines coverage ≥85% (`CI_FAIL_ON_COVERAGE_BREACH=true`).
- G4: Performance; metric — `gateway_calls/logical_charges` p99 ≤200 ms at 1k RPS local load.
- G5: Reliability; metric — exactly-once ratio 1.00±0.01 (15 m window) across 3 runs.
- G6: Observability; metric — `/metrics` (Prometheus) documented, secured (Auth + TLS), dev flag available.
- G7: Architecture; metric — zero cycles, intact layering, file/class/function limits satisfied.

## 3) Implementation Options & Trade-offs
- Option A: Pure Modular Monolith (recommended)
  - Pros: simple deployment, single binary, fast iteration, strict layers.
  - Cons: weaker module isolation, mitigated by architecture tests.
- Option B: Multi-crate workspace (split domains)
  - Pros: stronger isolation, parallel builds.
  - Cons: more complex releases/versioning, fragmentation risk.
- Option C: Plugin system for inspector
  - Pros: extensibility.
  - Cons: longer MVP, heavier API surface.
- Recommendation: Option A for now; revisit B/C post-GA if needed.

## 4) Scope / Non-Goals
- In scope:
  - MCP stdio server (rmcp 0.8.1) with inspector tools.
  - Clients for target MCPs: stdio, SSE, streamable HTTP.
  - Compliance suite v0.9, reporting, Prometheus metrics, JSON Schema events.
- Out of scope:
  - GUI/web UI.
  - Persistent database (use in-memory/file outbox for now).
  - Full-spectrum auth integrations for target servers (minimum: tokens/CA trust).

## 5) WBS (WS → Epics → Features → Tasks)
- WS-001 Architecture & Skeleton
  - Epic: DDD/Layers/Contracts → T-0001..T-0006, T-0036..T-0038
- WS-002 Target Transports
  - Epic: stdio/SSE/HTTP clients → T-0007..T-0010
- WS-003 Inspector Functionality
  - Epic: core operations → T-0011..T-0017, T-0030
- WS-004 Reliability & Side Effects
  - Epic: Idempotency/Outbox/Reaper → T-0014, T-0031..T-0033
- WS-005 Observability & SLO
  - Epic: Metrics/Logs/Tracing → T-0017..T-0019, T-0035
- WS-006 Contracts & Docs
  - Epic: Schemas/Public contracts → T-0020..T-0021, T-0027
- WS-007 Quality Gates
  - Epic: CI, mock servers, property/race tests → T-0022..T-0026, T-0038
- WS-008 Release & Ops
  - Epic: Releases/Security → T-0028..T-0034

## 6) Atomic Tasks (Specification)
Example (excerpt):
- id: T-0001; title: Build workspace skeleton; outcome: workspace + crate compile; inputs: `AGENTS.md`, requirements; outputs: workspace `Cargo.toml`, crate `mcp_multi_tool`; acceptance: `cargo build` succeeds on 3 OS; owner: RL; effort: 4 h.
- id: T-0005; title: Bootstrap stdio server; outcome: handshake OK; acceptance: integration handshake test passes.
- id: T-0016; title: Compliance suite; outcome: deterministic JSON/MD report; acceptance: suite passes baseline coverage.
- Full backlog aligns with `TODO.md` for day-to-day execution.

## 7) Timeline & Milestones
| Milestone | Window | Deliverable |
| --- | --- | --- |
| M-0001 Kickoff | Week 1 | Architecture skeleton, rmcp pin |
| M-0002 MVP Alpha | Week 2 | Stdio server handshake, basic inspector ops |
| M-0003 Beta | Week 3–5 | Compliance suite, metrics, SSE/HTTP clients |
| M-0004 GA | Week 5 | Release artifacts, docs, coverage gate |

## 8) Risk Register (Top 3)
| ID | Risk | Impact | Mitigation |
| --- | --- | --- | --- |
| R-001 | Upstream MCP spec change | Schedule slip | Track MCP updates weekly; keep feature flags ready |
| R-002 | Compliance coverage <95% | Blocks GA | Expand mock targets; add regression tests |
| R-003 | Metrics endpoint insecure | Observability gap | Enforce Auth + TLS; dev override gated by env |

## 9) Quality Gates Checklist
- All tasks have DoD/owner/dependencies assigned.
- Terminology consistent across docs and contracts.
- Goals are SMART and verifiable.
- Top risks have mitigation + contingency.
- Critical path realistic with buffer built in.

## 10) Next Actions (current iteration)
1. Lock plan IDs and dependency graph; confirm milestone dates.
2. Start T-0001/0002/0005: workspace skeleton + rmcp pin + stdio server bootstrap.
3. Implement T-0010 probe and T-0023 mock server for early E2E coverage.
