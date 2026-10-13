# Harness-Gate Capability Contracts

## ADDED Requirements

### Requirement: Runner configuration declares effective execution and isolation

The executor SHALL accept a versioned runner configuration that declares
effective arguments, argument insertion, environment inputs, result format, and
one of `shared`, `schema-per-worker`, or `database-per-worker` isolation modes.
It SHALL reject a test step with concurrency greater than one when no isolation
mode is declared, before starting workers or managed services.

#### Scenario: DevRail test configuration is accepted

- **WHEN** a valid DevRail test step declares runner threads, result format,
  environment mapping, and `schema-per-worker` isolation
- **THEN** config check succeeds
- **AND THEN** the machine result records the effective command, environment
  snapshot, thread limit, result format, and isolation mode

#### Scenario: Shared storage cannot be used implicitly in parallel

- **WHEN** a test step requests more than one worker with `shared` isolation or
  without an isolation declaration
- **THEN** validation fails before side effects
- **AND THEN** the failure identifies the step and required isolation change

#### Scenario: Worker cancellation cleans its isolation state

- **WHEN** one worker is cancelled or exits abnormally during migration or test
  execution
- **THEN** its schema/database/container state is marked terminal
- **AND THEN** the next invocation cannot reuse that state as a healthy worker

### Requirement: Each invocation has isolated, atomic evidence

Every invocation SHALL receive a non-reused `invocation_id` and an output
directory containing its invocation metadata, machine result, manifest, and
per-step attempt logs. Step IDs SHALL be stable while labels remain display
text. Paths SHALL be normalized, contained by the report root, collision-free,
and committed with temporary-file plus atomic-rename semantics.

#### Scenario: Concurrent invocations do not overwrite evidence

- **WHEN** two identical invocations run concurrently in one workspace
- **THEN** each writes a distinct invocation directory and step-attempt paths
- **AND THEN** neither invocation changes the other's report, logs, or manifest

#### Scenario: Report write failure is visible

- **WHEN** writing a required report or manifest fails
- **THEN** the invocation status is a report failure
- **AND THEN** it cannot be returned or published as `PASS`

### Requirement: Managed resources use cross-process leases

Managed resources SHALL have stable identities and owner markers. A lease SHALL
include invocation identity, process-start identity, creation time, heartbeat,
expiry, and resource kind. Acquire, renew, release, and reclaim SHALL be
idempotent, and cleanup SHALL be restricted to resources carrying a valid
Harness-Gate owner marker.

#### Scenario: Concurrent owners receive a deterministic conflict

- **WHEN** two independent processes request the same exclusive resource
- **THEN** exactly one obtains the lease
- **AND THEN** the other receives a structured conflict or bounded-wait result

#### Scenario: Killed owner is reclaimable

- **WHEN** the lease owner is forcibly terminated and stops heartbeating
- **THEN** the lease expires within its configured upper bound
- **AND THEN** doctor/cleanup dry-run identifies the resource and reclaiming it
  preserves an audit record

#### Scenario: Unmarked user resources are protected

- **WHEN** cleanup scans resources with similar names but no valid owner marker
- **THEN** those resources are left untouched
- **AND THEN** the result reports only the Harness-Gate-owned resources considered

### Requirement: Machine results use one versioned evidence schema

The executor SHALL emit a machine-readable result with `schema_version`,
invocation metadata, stable step IDs, statuses, attempts, timing, exit/cancel
reasons, configuration/tool summaries, artifact references, and an evidence
completeness indicator. `PASS`, `WAIVED`, `FAIL`, `SKIPPED`, and `CANCELLED`
SHALL be distinct. Parser failure, zero results, partial results, and report
write failure SHALL not be represented as an unconditional pass.

#### Scenario: Completion order does not change result order

- **WHEN** independent steps finish in different orders on repeated runs
- **THEN** normalized machine results list steps in validated DAG/configuration
  order
- **AND THEN** status and failure selection are equivalent for equivalent inputs

#### Scenario: DevRail consumes results without text parsing

- **WHEN** DevRail validates a result against a supported schema version
- **THEN** it can map scope, steps, warnings, failures, skipped/cancelled state,
  and artifacts to a quality-gate event
- **AND THEN** Markdown or raw log text is not required to determine status

### Requirement: Release artifacts are verifiable and attributable

Each release SHALL publish a checksum manifest, SPDX or CycloneDX SBOM, signed
asset/manifest/SBOM metadata, and provenance bound to repository, commit,
workflow, dependencies, and toolchain. Publication SHALL use pinned actions and
least-privilege permissions, and published assets SHALL not be overwritten
without an auditable new release.

#### Scenario: Clean environment verifies a release

- **WHEN** a consumer has only release assets, the published trust material,
  and the verification instructions
- **THEN** it can verify checksums, signature, SBOM integrity, and provenance
- **AND THEN** changing any byte causes verification to fail

### Requirement: Waivers are explicit, scoped, and expiring

The executor SHALL validate waiver identity, rule/step, scope, risk, reason,
owner, approval evidence, creation/expiry, compensating control, and revoke
state. Expired, out-of-scope, or self-approved waivers SHALL fail closed, and a
valid exception SHALL produce `WAIVED` rather than `PASS`.

#### Scenario: Expired waiver cannot release a gate

- **WHEN** a required step matches a waiver whose expiry is before invocation
  time
- **THEN** validation fails
- **AND THEN** the result is not `PASS` or `WAIVED`

#### Scenario: Valid waiver remains machine-distinct

- **WHEN** an authorized, in-scope waiver covers a failing step
- **THEN** the result records `WAIVED`, waiver ID, and approval evidence
- **AND THEN** a downstream required check can apply its own policy instead of
  treating the result as an unconditional pass

### Requirement: Test retries, flaky cases, and shards are replayable

The executor SHALL prefer standard test result formats and SHALL record parser
mode/version, each attempt, final status, flaky classification, shard identity,
merge identity, expected-failure policy, and failure-log references. Retry count
and retryable error classes SHALL be bounded and explicit; missing or duplicate
shards SHALL fail merge validation.

#### Scenario: Retry does not hide a security failure

- **WHEN** a test or security gate fails with a non-retryable error
- **THEN** no retry changes the terminal failure
- **AND THEN** the initial evidence and failure log remain linked to the result

#### Scenario: Shard merge is complete and non-duplicating

- **WHEN** all declared shards produce valid results with unique test identities
- **THEN** the merged result counts each test once and preserves attempt history
- **AND THEN** a missing or duplicate shard causes merge failure

### Requirement: Adapter expansion is out-of-process and versioned

Any future adapter protocol SHALL declare capabilities, input/configuration
version, result schema, timeout/cancellation behavior, logs/artifacts, and
resource/network/permission needs. Adapter loading SHALL validate protocol
version, source, and signature. Adapter crashes SHALL fail only their node and
cannot bypass dependency, redaction, cancellation, or evidence rules.

#### Scenario: Compatible adapter runs without recompiling the CLI

- **WHEN** a signed adapter advertises a supported protocol and result schema
- **THEN** Harness-Gate can execute it as an external node
- **AND THEN** its result uses the same status and artifact/redaction contract

#### Scenario: Adapter crash is isolated

- **WHEN** an adapter process exits unexpectedly
- **THEN** its node is failed with structured evidence
- **AND THEN** unrelated nodes, dependency propagation, cancellation, and logs
  remain governed by the scheduler contract

## Implementation Plan

1. Freeze baseline fixtures and approve the JSON schemas and version policy.
2. Implement invocation paths, runner/isolation validation, leases, and result
   publication with contract tests on Linux, macOS, and Windows.
3. Add release verification and clean-environment tamper tests.
4. Add waiver and test-result semantics with DevRail consumer fixtures.
5. Exercise shadow/canary rollout and rollback before changing required-check
   ownership.
6. Propose the adapter protocol separately after P0/P1 evidence is accepted.

## Rollback Plan

Disable the compatibility launcher and return required checks to the frozen
DevRail path. Revert the implementation as one reviewed change if the P0
contract suite, evidence-integrity checks, or cleanup proof fails. Rollback MUST
not delete invocation evidence, rewrite existing reports, or reclaim resources
without owner validation.
