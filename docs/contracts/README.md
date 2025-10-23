# Public Contracts

| Schema | Description |
| --- | --- |
| `inspection-run-event.schema.json` | Event emitted when an inspection run completes; intended for transactional outbox export. |
| `probe-result.schema.json` | Response envelope returned by the `inspector_probe` tool. |
| `call-result.schema.json` | Response envelope returned by the `inspector_call` tool. |

## Versioning
- Schemas follow semantic versioning via Git tags (`vX.Y.Z`).
- Breaking changes require a new major version and release notes entry.
- Keep `$id` URIs stable for published versions.

## Validation
Use `ajv` or `jsonschema` CLI to validate payloads:
```bash
npx ajv validate -s docs/contracts/probe-result.schema.json -d payload.json
```
