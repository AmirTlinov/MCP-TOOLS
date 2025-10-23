# Public Contracts

| Schema | Description |
| --- | --- |
| `inspection-run-event.schema.json` | Event emitted when an inspection run completes; intended for transactional outbox export. |
| `probe-result.schema.json` | Response envelope returned by the `inspector_probe` tool. |
| `call-result.schema.json` | Response envelope returned by the `inspector_call` tool. |
| `call-trace.schema.json` | Shape of the `_meta.trace` payload attached to `inspector_call` results. |

## Versioning
- Schemas follow semantic versioning via Git tags (`vX.Y.Z`).
- Breaking changes require a new major version and release notes entry.
- Keep `$id` URIs stable for published versions.

## Validation
Use `ajv` or `jsonschema` CLI to validate payloads:
```bash
npx ajv validate -s docs/contracts/probe-result.schema.json -d payload.json
```

### Streaming Call Results

When `inspector_call` runs with `{ "stream": true }`, the `structured_content` field is normalised into the shape:

```json
{
  "mode": "stream",
  "events": [
    { "event": "chunk", "progress": 1, "total": 2, "message": "chunk 1" },
    { "event": "final", "structured": {"status": "complete"} }
  ],
  "final": { "status": "complete" }
}
```

Each entry in `events` follows the `StreamEvent` definition inside `call-result.schema.json` and mirrors progress notifications emitted by the downstream MCP tool.

### Trace Metadata

Every `inspector_call` response enriches `CallToolResult._meta.trace` with a payload that matches `call-trace.schema.json`. It embeds the persisted `InspectionRunEvent`, records whether streaming was enabled, copies any captured `StreamEvent` notifications, and flags whether the transactional outbox write succeeded.
