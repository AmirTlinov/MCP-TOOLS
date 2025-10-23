# Metrics Specification

## Endpoint
- Path: `/metrics`
- Format: Prometheus text exposition
- Auth: Built-in Bearer token via `METRICS_AUTH_TOKEN`; disabled only when `ALLOW_INSECURE_METRICS_DEV=true`
- TLS: Native TLS when `METRICS_TLS_CERT_PATH` + `METRICS_TLS_KEY_PATH` provided, otherwise terminate in front of the binary

## Gauges
| Metric | Type | Description | Labels |
| --- | --- | --- | --- |
| `inspector_inflight` | gauge | Concurrent inspector operations across transports. | — |
| `outbox_backlog` | gauge | Total events persisted in the transactional outbox (JSONL or sqlite). | — |
| `error_budget_frozen` | gauge | 1 when the error budget freeze is active, otherwise 0. | — |

## Histograms
| Metric | Buckets | Description | Labels |
| --- | --- | --- | --- |
| `inspector_latency_ms` | default Prometheus buckets | Measured time for outbound operations, aligned with `gateway_calls/logical_charges`. | `operation`, `transport` |

## Counters
| Metric | Description | Trigger |
| --- | --- | --- |
| `idempotency_timeouts_total` | Count of inspection runs failed by the 60s reaper. | Incremented whenever the reaper marks an in-flight run as timed out. |

## Alerts
- **Outbox backlog**: fire when backlog > 1000 for >10m.
- **Latency p99**: alert when p99 > 200 ms for five consecutive windows.
- **Lock wait p99**: track via future gauge once concurrency primitives are instrumented.

## Scrape Example
```
# HELP mcp_multitool_latency_ms Time spent calling downstream MCP tools (ms)
# TYPE mcp_multitool_latency_ms histogram
mcp_multitool_latency_ms_bucket{operation="call",transport="stdio",le="50"} 4
...
```

## Testing
- Integration tests may stub `ALLOW_INSECURE_METRICS_DEV=true` to expose `/metrics` without TLS/Bearer.
- CI jobs may lint output using `promtool check metrics` to ensure exposition compatibility.
