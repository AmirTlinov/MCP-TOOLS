# Contributing Guide

Thanks for helping keep MCP TOOLS at flagship quality. Follow the rules below for every change.

## Principles
- **DDD + Modular Monolith**: add new crates under `tools/`. Respect layer boundaries inside crates (`domain`, `app`, `adapters`, `infra`, `shared`).
- **Config First**: place configuration defaults under `config/` and document them.
- **Contract First**: publish JSON Schemas for every public API or event under `docs/contracts/` and link them from the README when ready.

## Quality Gates
- `cargo fmt` — required formatting.
- `cargo clippy --all-targets --all-features -D warnings` — no warnings allowed.
- `cargo test` — run unit and integration suites.
- `cargo llvm-cov --fail-under-lines 85` — line coverage must stay at or above 85% for the touched code.
- State machines (`pending → processing → captured → failed`) need property tests to prove illegal transitions are blocked.

## Git & CI
- Branch names: `feature/<slug>` or `fix/<slug>`.
- Follow Conventional Commits (`feat:`, `fix:`, `chore:`, etc.).
- Every PR must pass `.github/workflows/ci.yml` (fmt, clippy, test, coverage).

## Safety & Reliability
- Execute side effects with the `CLAIM|OUTBOX` template and `UPDATE … RETURNING` semantics.
- Register metrics via `infra::metrics` and document them in `docs/metrics.md`.
- Never hardcode secrets; rely on dotenv/config wiring.

## Documentation
- Update `AGENTS.md`, `PLAN.md`, and `README.md` when strategy or scope changes.
- Add `docs/<tool>/overview.md` for each new tool, covering architecture and usage scenarios.

Thanks for contributing!
