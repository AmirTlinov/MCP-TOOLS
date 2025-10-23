# Metrics Specification

## Endpoint
- Path: `/metrics`
- Format: Prometheus text exposition
- Auth: Basic or Bearer as defined by reverse proxy; disabled only when `ALLOW_INSECURE_METRICS_DEV=true`
- TLS: Required in production; run behind TLS-terminating proxy or enable native TLS (future work)

## Gauges
| Metric | Type | Description | Labels |
| --- | --- | --- | --- |
| `mcp_multitool_pending_gauge` | gauge | Number of in-flight inspector operations (probe/list/call). | `operation` (probe|list|call) |

## Histograms
| Metric | Buckets | Description | Labels |
| --- | --- | --- | --- |
| `mcp_multitool_latency_ms` | [50, 100, 200, 500, 1000, +Inf] | Measured time for outbound operations, aligned with `gateway_calls/logical_charges`. | `operation`, `transport` |

## Counters (planned)
| Metric | Description | Trigger |
| --- | --- | --- |
| `mcp_multitool_outbox_replays_total` | Count of replayed outbox entries after failures. | Emitted once transactional outbox is wired. |

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
- Integration tests should stub `ALLOW_INSECURE_METRICS_DEV=true` to expose `/metrics` without TLS.
- CI jobs may lint output using `promtool check metrics` once outbox metrics land.
