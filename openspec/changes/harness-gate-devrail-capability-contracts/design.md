# Design: Harness-Gate and DevRail Capability Contracts

## Scope Boundary

Harness-Gate is the deterministic execution and evidence layer. DevRail is the
business control plane that selects policy, authorizes data, owns organization
audit, and decides how a machine result affects a change or deployment. The
boundary is a versioned event and artifact contract, not a shared database or
implicit convention.

| Boundary | Harness-Gate contract | DevRail responsibility |
| --- | --- | --- |
| Invocation | Create an `invocation_id`, capture commit/platform/executor metadata, and isolate all outputs | Supply a correlation/request ID and retain events according to policy |
| Step execution | Validate runner config, resolve dependencies, execute, cancel, and emit structured status | Select the gate policy and interpret status for the organization |
| Resources | Allocate owner-marked resources, renew/release leases, and report cleanup | Supply permitted resource classes and escalation policy |
| Evidence | Write redacted command/environment summaries, logs, manifests, and result schema | Store/query artifacts and enforce access controls |
| Exceptions | Emit explicit `WAIVED` only when a valid waiver is supplied | Approve, revoke, audit, and expire waivers |
| Publication | Build and verify checksums/SBOM/signatures/provenance inputs | Own release trust roots and required-check/deployment decisions |

## Phase 0: Baseline and Compatibility Launcher

Before changing execution, capture a versioned serial baseline containing the
selected scope, effective configuration, command arguments, environment keys
(values redacted), step order, exit/status mapping, reports, logs, cleanup
observations, commit SHA, platform, and executor version. A compatibility
launcher accepts the existing DevRail request and translates it into the new
invocation contract without changing the serial default.

The launcher rejects unknown contract versions and records both the source
request ID and the generated `invocation_id`. It must be possible to replay a
baseline request against a fixed checkout without network access to DevRail.

## P0 Contracts

### Runner arguments and isolation

The v2 runner configuration is explicit and versioned. A representative shape
is:

```toml
[steps.backend_tests.runner]
kind = "cargo-test"
threads = 4
threads_env = "RUST_TEST_THREADS"
args = ["--all-features"]
result_format = "junit"
isolation = "schema-per-worker"
```

The executor records the effective command, argument insertion location,
declared environment variables, result format, and isolation mode. `shared`,
`schema-per-worker`, and `database-per-worker` are distinct modes. A parallel
test step without an isolation declaration is rejected before services or
workers start. Migration initialization, public-schema locks, cleanup failure,
and worker cancellation are terminal states in the result protocol rather than
implicit retries.

### Invocation evidence and artifact paths

Each invocation owns a directory below the configured report root:

```text
reports/<invocation_id>/
  invocation.json
  result.json
  manifest.json
  steps/<step_id>/attempt-<n>/stdout.log
  steps/<step_id>/attempt-<n>/stderr.log
```

`invocation_id` is generated once and is not reused. Step IDs are stable across
attempts; labels are display text only. Paths are normalized and preallocated,
with collision and report-root containment checks. Files are written to a
temporary sibling and atomically renamed. A report or manifest write failure
is an invocation failure and cannot be reported as `PASS`.

Every step record includes `step_id`, `attempt`, parent invocation, start/end
timestamps, duration, exit code or signal, timeout/cancellation reason,
configuration/tool summary, and log/artifact references. Exported commands,
environment, and logs pass the existing DevRail redaction policy; secrets,
tokens, cookies, private keys, connection strings, and complete request headers
are never emitted.

### Resource leases and orphan recovery

Managed containers, databases, ports, workspaces, and report directories have
stable resource identities and owner markers. A lease contains at least:
`invocation_id`, process-start identity (not only PID), creation time,
heartbeat, expiry, and resource kind. Acquire, renew, release, and reclaim are
idempotent. PID reuse, host restart, partial startup, and a stale heartbeat
cannot transfer ownership to a new invocation without validation.

The runtime exposes a diagnostic cleanup operation with a dry-run mode. It may
reclaim only resources carrying a valid Harness-Gate owner marker. Conflicts,
lease expiry, and reclaim failures are structured results and remain visible to
CI retry and human remediation.

### Unified result and evidence protocol

The machine result is versioned independently from Markdown/HTML rendering and
is defined by the checked-in [machine-result JSON Schema](../../../schema/machine-result.schema.json):

```json
{
  "schema_version": "1",
  "invocation_id": "inv-20260830-01HX...",
  "executor_version": "0.4.0",
  "report_directory": ".harness-gate/reports/invocations/inv-20260830-01HX...",
  "timestamp": "2026-08-30T00:00:00Z",
  "profile": "full",
  "scope": {"mode": "all", "changed_files": [], "components": ["backend"], "unmatched_files": []},
  "services": [],
  "steps": [{
    "step_id": "backend.tests",
    "invocation_id": "inv-20260830-01HX...",
    "label": "backend tests",
    "passed": false,
    "status": "FAIL",
    "timed_out": false,
    "cancelled": false,
    "duration_ms": 1200,
    "log": ".harness-gate/reports/invocations/inv-20260830-01HX.../logs/backend.log",
    "detail": "exit code 101",
    "runner": null,
    "attempts": [{
      "attempt": 1,
      "status": "FAIL",
      "started_at": "2026-08-30T00:00:00Z",
      "finished_at": "2026-08-30T00:00:01Z",
      "duration_ms": 1200,
      "timed_out": false,
      "cancelled": false,
      "log": ".harness-gate/reports/invocations/inv-20260830-01HX.../logs/backend.log",
      "detail": "exit code 101"
    }]
  }],
  "skipped_steps": [],
  "warnings": [],
  "failures": [{"step_id": "backend.tests", "code": "STEP_FAILED", "message": "exit code 101"}],
  "artifacts": [{"path": "logs/backend.log", "kind": "step-log"}],
  "evidence_complete": true,
  "passed": false,
  "status": "FAIL"
}
```

The schema defines `PASS`, `WAIVED`, `FAIL`, `SKIPPED`, and `CANCELLED` without
using human-readable detail to infer status. Scope, services, warnings,
failures, artifacts, input/config snapshots, and evidence completeness use
stable fields. Parser failure, zero results, partial results, and report-write
failure have separate error codes. Result ordering is the validated DAG/config
order, independent of worker completion order. JSON Schema, fixtures, and a
compatibility policy (backward-compatible additions within a major schema,
explicit migration for breaking changes) are checked in with the implementation.

### Release integrity and provenance

Every release asset has a checksum manifest, SPDX or CycloneDX SBOM, signed
manifest/SBOM/asset metadata, and provenance binding repository, commit, build
workflow, dependencies, and toolchain. Actions, toolchains, and release
permissions are pinned and least-privilege. Existing assets are immutable after
publication. Installation and upgrade docs include offline verification, and a
controlled DevRail launcher verifies version and digest before execution.

## P1 Contracts

### Waiver and exception governance

A waiver contains `waiver_id`, rule/step, scope, risk, reason, owner,
approval evidence, created/expiry timestamps, compensating control, and revoke
state. The executor rejects expired, out-of-scope, or self-approved waivers.
Results mark a waiver as `WAIVED`; DevRail remains responsible for approval,
RBAC, immutable audit events, and organization-level queries.

### Standard test-result semantics

JUnit, TRX, or stable JSON is preferred over regex log counting. Regex remains a
compatibility parser and must report its mode and confidence. Each test case
records attempts, final status, flaky classification, shard ID, merge identity,
expected-failure policy, and failure-log references. Retry has a bounded count,
an explicit backoff, and an allowlist of retryable infrastructure errors; it
cannot hide security or gate failures. Missing or duplicate shards fail merge
validation.

## P2 Adapter Protocol

The follow-up adapter proposal should define an out-of-process protocol for
configuration input, capability declaration, result schema, timeout/cancel,
logs, artifacts, resource/network/permission needs, signing, compatibility
matrix, crash isolation, upgrade, and rollback. Adapter failures are confined
to their node and cannot bypass dependencies, redaction, cancellation, or
evidence requirements. Dynamic libraries are not loaded into the main process
without a separate trust-boundary decision.

## Rollout, Observability, and Rollback

The launcher runs the old and new paths in shadow mode with normalized result
comparison. Differences are classified as expected policy changes, contract
bugs, or environment variance; raw evidence is retained for the audit window.
A canary enables the new path for one bounded repository/team slice while
DevRail remains the required-check owner. Promotion requires equivalent or
explicitly approved results, no lease/artifact leaks, and successful offline
release verification. Rollback disables the launcher flag and returns to the
frozen DevRail path without deleting reports or managed resources.

Metrics include invocation and step durations, retry/flaky counts, lease
conflicts/expiry/reclaims, cleanup failures, report-write failures, evidence
completeness, schema version, and shadow divergence. Logs and metrics follow
DevRail redaction and retention policy.

## Implementation Plan and Timeline

The exact calendar depends on the owning teams; the sequence is normative:

| Phase | Scope | Exit evidence |
| --- | --- | --- |
| 0 | Baseline, schemas, launcher, path model | Replayed serial fixtures and reviewed contract schemas |
| 1 | P0 runner, invocation evidence, leases, result protocol, release verification | Cross-platform contract suite and clean-environment tamper tests |
| 2 | P1 waivers and test-result semantics | Expiry/scope/retry/shard fixtures and DevRail consumer tests |
| 3 | Shadow and canary rollout | Equivalent normalized results, no leaks, tested rollback |
| 4 | P2 adapter protocol | Separate ADR/OpenSpec approval and signed adapter contract tests |

## Alternatives Considered

- Infer behavior from command strings: rejected because it is not declarative or
  auditable.
- Use a shared report directory with timestamps: rejected because concurrent
  calls can still collide and cannot establish parent/attempt identity.
- Treat resource cleanup as best effort: rejected because a stale database,
  port, or workspace can contaminate a later gate.
- Emit only a final boolean: rejected because required checks need to distinguish
  failures, waivers, cancellations, partial evidence, and parser errors.
