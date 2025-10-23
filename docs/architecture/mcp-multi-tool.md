# MCP MultiTool Architecture

```
+--------------------+        +------------------+
|   shared (utils)   |<-------| domain (run SM)  |
+--------------------+        +------------------+
          ^                          ^
          |                          |
+--------------------+        +------------------+
|     app layer      |<-------| adapters (stdio) |
+--------------------+        +------------------+
          ^                          ^
          |                          |
     +---------+                +---------+
     |  infra  |                |  entry  |
     +---------+                +---------+
```

## Layers
- **shared**: pure helpers, DTOs, and schema types reused across layers.
- **domain**: `InspectionRun` state machine guarding allowed transitions (`pending → processing → captured/failed`).
- **app**: orchestrates inspector use cases (probe, list, call). Handles transport selection and latency metrics.
- **app::compliance**: reusable suite that drives probe/list/call checks against downstream MCP servers and emits deterministic reports.
- **adapters**: RMCP server wiring, request/response mapping, help manifest.
- **infra**: configuration reader, Prometheus metrics server.
- **entry**: `main.rs` bootstrap, logging setup, metrics spawn, RMCP stdio service start.

## Side-Effect Strategy
- Execute remote tool calls through the app layer, producing `CallToolResult` while tracking `InspectionRun` invariants.
- Outbox (file-based) stores DLQ events (`data/outbox/dlq.jsonl`) via `infra::outbox::Outbox`. The `infra` module will own durable outbox wiring in future milestones.
- Each effect path obeys `CLAIM|OUTBOX`: claim run, perform effect, persist event if downstream is unavailable.

## Metrics Flow
1. `app::inspector_service` wraps outbound operations with `PendingGaugeGuard` to expose queue depth (`mcp_multitool_pending_gauge`).
2. Latency measurements feed `LATENCY_HISTO` (Prometheus histogram) for `gateway_calls/logical_charges` alignment.
3. `/metrics` endpoint (Axum) leverages TLS/Auth gating via `AppConfig` (env-driven).

## Configuration Surfaces
- `AppConfig` reads environment (dotenv fallback). All runtime knobs reside under `infra::config`.
- Dev override flag `ALLOW_INSECURE_METRICS_DEV` relaxes TLS/auth only in non-production contexts.

## Future Hooks
- Add SSE/HTTP clients under `app::inspector_service` as feature flags mature.
- Promote file-based outbox to a durable backend with replay worker.
