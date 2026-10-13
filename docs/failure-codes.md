# Failure Codes

Harness-Gate serializes machine failures with an uppercase `FailureCode` value.
The registry is closed in the current schema: a new producer must add a code
and update this table before it can publish a machine result. Human detail text
is explanatory only and is never used for retry or status decisions.

| Code | Meaning | Retry class |
| --- | --- | --- |
| `WEBHOOK_DESTINATION_DENIED` | URL host or resolved address is outside the egress policy | `exit` |
| `WEBHOOK_REDIRECT_DENIED` | Webhook returned a redirect while redirects are disabled | `exit` |
| `LEASE_OWNERSHIP_UNCERTAIN` | Lease identity or renewal cannot be proven | `exit` |
| `SERVICE_SETUP_FAILURE` | Service configuration or startup setup failed | `exit` |
| `SERVICE_LEASE_FAILURE` | A service lease could not be acquired | `exit` |
| `RESULT_PARSE_FAILURE` | Configured test result could not be parsed | `parser` |
| `RESULT_ZERO` | A parser found no results | `parser` |
| `RESULT_PARTIAL` | A parser found fewer results than required | `parser` |
| `SCHEDULER_FAILURE` | The verification scheduler or cleanup failed | `exit` |
| `SECRET_SCAN_FAILURE` | Secret scan found a blocking finding | `exit` |
| `QUALITY_BLOCKED` | Generic quality policy failed or trusted workflow inputs/evidence are invalid | `exit` |
| `ARCHITECTURE_AUDIT_FAILURE` | Architecture audit found a blocking violation | `exit` |
| `STEP_EXECUTION_FAILURE` | A configured step could not execute | `exit` |
| `STEP_SKIPPED` | A step was not dispatched because a prerequisite failed | `exit` |
| `OUTPUT_LIMIT_EXCEEDED` | A bounded process reader exceeded its byte budget | `exit` |
| `READER_DEADLINE_EXCEEDED` | A process reader exceeded its completion deadline | `timeout` |
| `STEP_CANCELLED` | The run was cancelled before completion | `cancelled` |
| `STEP_TIMEOUT` | A step exceeded its timeout | `timeout` |
| `STEP_FAILED` | A step exited unsuccessfully without a more specific code | `exit` |
| `EVIDENCE_PATH_ESCAPE` | An artifact or step log escapes the invocation root | `exit` |
| `EVIDENCE_PENDING` | Evidence publication has not reached a complete state | `exit` |
| `EVIDENCE_FINALIZATION_FAILURE` | Evidence finalization failed before the machine result could be published | `exit` |
| `EVIDENCE_PUBLICATION_FAILURE` | A machine or human report could not be published atomically | `exit` |
| `EVIDENCE_DUPLICATE_PATH` | Multiple artifact bindings claim the same path | `exit` |
| `EVIDENCE_STEP_UNBOUND` | An artifact is not bound to a declared verification step | `exit` |
| `EVIDENCE_INVOCATION_MISMATCH` | An artifact or manifest belongs to another invocation | `exit` |
| `EVIDENCE_MISSING` | A required artifact or manifest entry is missing | `exit` |
| `EVIDENCE_UNDECLARED_FILE` | A file was found in the evidence root without a declaration | `exit` |
| `EVIDENCE_SYMLINK` | A symlink was found where a regular evidence file was required | `exit` |
| `EVIDENCE_INVALID_TYPE` | An evidence entry has an unsupported type or shape | `exit` |
| `EVIDENCE_READ_FAILURE` | An evidence file could not be read safely | `exit` |
| `EVIDENCE_INVALID_METADATA` | Evidence metadata is malformed or inconsistent | `exit` |

Retry policies use the closed lowercase classes `cancelled`, `timeout`,
`parser`, and `exit`. Existing machine-result consumers can continue reading
the string fields; the spelling above is the compatibility contract. Unknown
codes or retry classes at the publication boundary fail closed and leave an
incomplete evidence result.

## Compatibility

| Existing field | Wire spelling | Producer contract |
| --- | --- | --- |
| `steps[].failure_code` | Existing uppercase code strings | Keep the field and use a value from the closed registry above. |
| `steps[].retry_class` | Existing lowercase class strings | Keep the field and use one of `cancelled`, `timeout`, `parser`, or `exit`. |
| `failures[].code` | Uppercase registry value | New failure records must use a registered `FailureCode`; human detail is supplementary. |
| Unknown code or class | Rejected | Publication fails closed and retains incomplete evidence for inspection. |
