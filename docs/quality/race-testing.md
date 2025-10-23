# Error Budget & Lock Contention Test Plan (T-0025)

## Goals
- Verify inspector service maintains lock wait p99 ≤ 50 ms across concurrent workloads.
- Ensure idempotency store, outbox, and error-budget tracker remain starvation-free under load.

## Target Components
1. `IdempotencyStore` mutexes (`records`, `external_refs`).
2. `Outbox::write_lock` guarding file/sqlite append.
3. Error budget state mutex.

## Instrumentation Strategy
- Wrap each critical mutex acquisition with `tracing` span + histogram (`lock_wait_ms`).
- Use `parking_lot::Mutex::try_lock_for` fallback in tests to capture wait durations without altering production code paths.
- Export Prometheus histogram `inspector_lock_wait_ms` (labels: component).

## Test Harness Outline
1. **Synthetic Load (Rust test)**
   - Spawn N=128 async tasks issuing `inspector_call` against an in-memory mock transport (reusing mock server). Use deterministic delay injection to simulate contention.
   - Record lock wait metrics; assert p99 ≤ 50 ms.

2. **Property Test (proptest)**
   - Randomize sequence of `claim/complete` operations with concurrent tasks.
   - Fail test if any sampled wait > 50 ms or if starvation detected (task timed out).

3. **Regression Guard**
   - Add `tests/lock_wait.rs` executing within 2s wall-clock: collects histogram snapshot and compares against config threshold.

## Acceptance Criteria
- Histograms logged in tests show p99 ≤ 50 ms for `idempotency`, `outbox`, `error_budget` locks.
- Prometheus metric `inspector_lock_wait_ms` exposed when instrumentation enabled.
- Tests fail fast when contention breaches SLO.
