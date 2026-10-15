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

## Query an explanation

```sh
harness-gate explain QUALITY_BLOCKED
harness-gate explain RESULT_PARSE_FAILURE --json
```

This command is read-only and does not discover a project, load workflow
configuration, run checks or apply changes. It accepts the exact uppercase machine
codes in the registry above; unknown codes return a nonzero exit status. The CLI `E...` codes below are also supported. Configuration `HGCFG-...` codes
remain a separate diagnostic family with inline help and are not yet accepted by this command. JSON output has `code`, `title`, `help` and
`documentation` fields. Advice is supplementary; retry classes, required checks
and final status remain unchanged. This is one part of issue #64, not delivery of
interactive edits or all diagnostic families.

## webhook-destination-denied

**WEBHOOK_DESTINATION_DENIED — URL host or resolved address is outside the egress policy.**

Check the configured destination and egress policy. Use an approved endpoint; do not disable address or redirect checks.

## webhook-redirect-denied

**WEBHOOK_REDIRECT_DENIED — Webhook returned a redirect while redirects are disabled.**

Check the configured destination and egress policy. Use an approved endpoint; do not disable address or redirect checks.

## lease-ownership-uncertain

**LEASE_OWNERSHIP_UNCERTAIN — Lease identity or renewal cannot be proven.**

Inspect the recorded owner and runtime identity. Use cleanup --dry-run to inspect stale leases; retain uncertain resources rather than deleting by name.

## service-setup-failure

**SERVICE_SETUP_FAILURE — Service configuration or startup setup failed.**

Inspect the service setup log and required tools or credentials. Confirm resource ownership before cleanup, then retry the original verification.

## service-lease-failure

**SERVICE_LEASE_FAILURE — A service lease could not be acquired.**

Inspect the service setup log and required tools or credentials. Confirm resource ownership before cleanup, then retry the original verification.

## result-parse-failure

**RESULT_PARSE_FAILURE — Configured test result could not be parsed.**

Inspect the declared result format, output path and parser settings. Ensure the producing test command completed and emitted the required results.

## result-zero

**RESULT_ZERO — A parser found no results.**

Inspect the declared result format, output path and parser settings. Ensure the producing test command completed and emitted the required results.

## result-partial

**RESULT_PARTIAL — A parser found fewer results than required.**

Inspect the declared result format, output path and parser settings. Ensure the producing test command completed and emitted the required results.

## scheduler-failure

**SCHEDULER_FAILURE — The verification scheduler or cleanup failed.**

Inspect the first scheduler or cleanup failure and its step logs. Resolve the underlying error before rerunning verification.

## secret-scan-failure

**SECRET_SCAN_FAILURE — Secret scan found a blocking finding.**

Inspect the finding without printing the secret. Remove exposed credentials from tracked inputs and rotate compromised values before rerunning the scan.

## quality-blocked

**QUALITY_BLOCKED — Generic quality policy failed or trusted workflow inputs/evidence are invalid.**

Inspect the normalized findings and evidence-validation errors. Repair missing or invalid evidence, improve tested coverage or simplify risky code; preserve required gates and thresholds.

## architecture-audit-failure

**ARCHITECTURE_AUDIT_FAILURE — Architecture audit found a blocking violation.**

Inspect the reported rule and source location. Correct the dependency or boundary violation, then rerun the audit.

## step-execution-failure

**STEP_EXECUTION_FAILURE — A configured step could not execute.**

Inspect the step log, command, dependencies and execution environment. Correct the first failure before rerunning; a skipped or cancelled step is not a pass.

## step-skipped

**STEP_SKIPPED — A step was not dispatched because a prerequisite failed.**

Inspect the step log, command, dependencies and execution environment. Correct the first failure before rerunning; a skipped or cancelled step is not a pass.

## output-limit-exceeded

**OUTPUT_LIMIT_EXCEEDED — A bounded process reader exceeded its byte budget.**

Reduce unnecessary command output or correct runaway logging. Inspect the explicit output budget; truncated output cannot establish a successful result.

## reader-deadline-exceeded

**READER_DEADLINE_EXCEEDED — A process reader exceeded its completion deadline.**

Check whether child processes still hold stdout or stderr open. Ensure descendants terminate and close inherited pipes before retrying.

## step-cancelled

**STEP_CANCELLED — The run was cancelled before completion.**

Inspect the step log, command, dependencies and execution environment. Correct the first failure before rerunning; a skipped or cancelled step is not a pass.

## step-timeout

**STEP_TIMEOUT — A step exceeded its timeout.**

Inspect the step log, command, dependencies and execution environment. Correct the first failure before rerunning; a skipped or cancelled step is not a pass.

## step-failed

**STEP_FAILED — A step exited unsuccessfully without a more specific code.**

Inspect the step log, command, dependencies and execution environment. Correct the first failure before rerunning; a skipped or cancelled step is not a pass.

## evidence-path-escape

**EVIDENCE_PATH_ESCAPE — An artifact or step log escapes the invocation root.**

Inspect the affected artifact binding and invocation identity. Correct the producer or path and generate fresh evidence; do not edit manifests to make stale or missing evidence pass.

## evidence-pending

**EVIDENCE_PENDING — Evidence publication has not reached a complete state.**

Inspect the affected artifact binding and invocation identity. Correct the producer or path and generate fresh evidence; do not edit manifests to make stale or missing evidence pass.

## evidence-finalization-failure

**EVIDENCE_FINALIZATION_FAILURE — Evidence finalization failed before the machine result could be published.**

Inspect the affected artifact binding and invocation identity. Correct the producer or path and generate fresh evidence; do not edit manifests to make stale or missing evidence pass.

## evidence-publication-failure

**EVIDENCE_PUBLICATION_FAILURE — A machine or human report could not be published atomically.**

Inspect the affected artifact binding and invocation identity. Correct the producer or path and generate fresh evidence; do not edit manifests to make stale or missing evidence pass.

## evidence-duplicate-path

**EVIDENCE_DUPLICATE_PATH — Multiple artifact bindings claim the same path.**

Inspect the affected artifact binding and invocation identity. Correct the producer or path and generate fresh evidence; do not edit manifests to make stale or missing evidence pass.

## evidence-step-unbound

**EVIDENCE_STEP_UNBOUND — An artifact is not bound to a declared verification step.**

Inspect the affected artifact binding and invocation identity. Correct the producer or path and generate fresh evidence; do not edit manifests to make stale or missing evidence pass.

## evidence-invocation-mismatch

**EVIDENCE_INVOCATION_MISMATCH — An artifact or manifest belongs to another invocation.**

Inspect the affected artifact binding and invocation identity. Correct the producer or path and generate fresh evidence; do not edit manifests to make stale or missing evidence pass.

## evidence-missing

**EVIDENCE_MISSING — A required artifact or manifest entry is missing.**

Inspect the affected artifact binding and invocation identity. Correct the producer or path and generate fresh evidence; do not edit manifests to make stale or missing evidence pass.

## evidence-undeclared-file

**EVIDENCE_UNDECLARED_FILE — A file was found in the evidence root without a declaration.**

Inspect the affected artifact binding and invocation identity. Correct the producer or path and generate fresh evidence; do not edit manifests to make stale or missing evidence pass.

## evidence-symlink

**EVIDENCE_SYMLINK — A symlink was found where a regular evidence file was required.**

Inspect the affected artifact binding and invocation identity. Correct the producer or path and generate fresh evidence; do not edit manifests to make stale or missing evidence pass.

## evidence-invalid-type

**EVIDENCE_INVALID_TYPE — An evidence entry has an unsupported type or shape.**

Inspect the affected artifact binding and invocation identity. Correct the producer or path and generate fresh evidence; do not edit manifests to make stale or missing evidence pass.

## evidence-read-failure

**EVIDENCE_READ_FAILURE — An evidence file could not be read safely.**

Inspect the affected artifact binding and invocation identity. Correct the producer or path and generate fresh evidence; do not edit manifests to make stale or missing evidence pass.

## evidence-invalid-metadata

**EVIDENCE_INVALID_METADATA — Evidence metadata is malformed or inconsistent.**

Inspect the affected artifact binding and invocation identity. Correct the producer or path and generate fresh evidence; do not edit manifests to make stale or missing evidence pass.

## CLI error explanations

Human CLI failures retain their existing code and exit status and now print an
`explain` command. These codes are not additions to the machine FailureCode registry.

## e1000

**E1000 — Command invocation failed.**

Read the accompanying command error. Check arguments, explicit input paths and required files; use --help for the failing command. No successful verification is implied.

## e1101

**E1101 — Architecture audit configuration failed.**

Inspect the audit configuration diagnostic and fix the named rule or path before rerunning audit.

## e1102

**E1102 — Architecture audit execution failed.**

Inspect the audit error and scan inputs. Correct the execution or access problem before rerunning the required audit.

## e1103

**E1103 — Audit log parsing failed.**

Check the input log path and supported format for parse-logs; retain the original log when correcting the producer.

## e1201

**E1201 — Secret scan configuration failed.**

Correct the named secret-scan configuration error. Keep required scanning enabled and avoid printing secret values.

## e1202

**E1202 — Secret scan failed.**

Inspect the scan error or finding, correct unreadable inputs, and remove or rotate exposed credentials as applicable. Do not bypass required scanning.

## e1301

**E1301 — Git scope detection failed.**

Check the selected repository, Git availability and requested revision or staged snapshot. Retry with the intended scope after correcting the Git error.

## e1302

**E1302 — Scope configuration failed.**

Inspect the scope rule diagnostic and correct invalid patterns, components or configuration before rerunning scope.

## e1303

**E1303 — Changed files do not match scope rules.**

Review each unmatched path and assign it to the correct component with reviewed scope rules. Do not exclude changed files merely to pass verification.

## e1304

**E1304 — Scope report failed.**

Check the report destination and permissions. Correct the write failure and regenerate the scope report.

## e1401

**E1401 — Verification profile or step is invalid.**

Check the configured profile and step identifiers and whether the step belongs to the selected profile. Use verify --plan to inspect the corrected execution plan.

## e1402

**E1402 — Verification was cancelled.**

Identify the cancellation source and confirm cleanup outcomes. Rerun the intended verification after cancellation is resolved; cancelled work is not successful evidence.

## e1403

**E1403 — Verification execution failed.**

Inspect the first execution error and step logs. Correct the command, environment or prerequisite failure, then generate fresh verification evidence.

## e1404

**E1404 — Verification report failed.**

Check the report destination, free space, permissions and evidence finalization error. Preserve incomplete evidence for diagnosis and rerun after correcting the cause.
