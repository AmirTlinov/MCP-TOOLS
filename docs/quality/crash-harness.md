# Crash & Edge Harness Plan (T-0026)

## Objectives
- Validate MCP MultiTool recovers gracefully from downstream crashes, I/O failures, and process interruptions.
- Guarantee transactional outbox durability and error-budget state integrity across unexpected exits.

## Scenarios
1. **Downstream Transport Abort**
   - Force mock server to exit mid-stream (stdio/SSE/HTTP). Expect `inspector_call` to emit structured error, record failure event, maintain idempotency state.
2. **Outbox Write Failure**
   - Simulate disk full/permission error (tempdir with read-only permission). Verify DLQ fallback and error log.
3. **Process Crash During Append**
   - Use harness to fork child process executing `call_stdio`; send SIGKILL post lock acquisition. On restart, confirm sqlite/file outbox remains consistent and reaper handles stranded keys.
4. **Error-Budget State Persistence**
   - Crash after freeze triggered; ensure restart maintains frozen window (persist via outbox event).

## Harness Implementation
- Standalone binary `tests/crash_harness.rs` orchestrating child processes with deterministic signals.
- Use temp directories for isolation, capturing logs + outbox contents.
- Assertions:
  - No panics; JSON schemas validated against `docs/contracts/*`.
  - On restart, first `inspector_call` respects prior freeze state if window active.

## Tooling
- Leverage `assert_cmd` and `duct` for process orchestration.
- Use `serde_json` diff to confirm outbox entries remain exactly-once.

## Acceptance Criteria
- Each scenario automated in CI (feature flag to skip on unsupported OS).
- Final report summarises crash outcomes and ensures zero event loss.
