# Post-Remediation Hardening

This specification is proposed; selected runtime tracks are implemented
incrementally on this branch, but the umbrella has no production or staging
acceptance claim.

## ADDED Requirements

### Requirement: Adapter isolation claims remain bounded

The project documentation and adapter contract SHALL describe capability
allowlists, protocol validation, bounded readers, and process-group cleanup as
protocol or lifecycle controls. They SHALL NOT claim operating-system network,
filesystem, resource, or complete descendant isolation until a separate
platform-sandbox decision is accepted with platform evidence.

#### Scenario: Current adapter wording is reviewed

- **WHEN** documentation consistency checks inspect adapter, README, ADR, and
  OpenSpec wording
- **THEN** the current claims identify protocol/process-group boundaries
- **AND THEN** no current document presents them as an OS sandbox

#### Scenario: A future sandbox is proposed

- **WHEN** a future change proposes OS enforcement
- **THEN** it defines platform primitives, identity, resource scope, failure
  behavior, and cross-platform fixtures in a separate ADR/OpenSpec

### Requirement: Webhook destinations use fail-closed egress policy

Webhook sending SHALL require an explicit host allowlist and SHALL reject
loopback, RFC1918/private, link-local, unspecified, multicast, and other
local-only resolved addresses by default. The sender SHALL re-resolve or
re-check every destination immediately before connection and SHALL reject a
policy change caused by DNS rebinding. Redirect behavior SHALL be disabled or
revalidated using the same policy.

#### Scenario: Allowlisted public destination succeeds

- **WHEN** a configured URL has an allowlisted host and all addresses resolved
  immediately before connection are public and permitted
- **THEN** the sender may connect
- **AND THEN** the emitted evidence contains only a redacted destination summary

#### Scenario: Private or loopback destination is configured

- **WHEN** a URL resolves to loopback, RFC1918/private, link-local, unspecified,
  or multicast space
- **THEN** validation or sending fails with a structured destination-denied
  result
- **AND THEN** no connection attempt is authorized

#### Scenario: DNS changes after initial validation

- **WHEN** a previously permitted host resolves to a denied address before
  connection
- **THEN** the connection fails closed
- **AND THEN** the failure does not become a successful retry through the old
  address decision

### Requirement: Leases remain attributable throughout long steps

Lease liveness SHALL cover allocation, execution, cancellation, and cleanup on
all supported platforms. Renewal SHALL occur before the TTL and SHALL carry
reliable platform identity. When identity or renewal cannot be proven, the
system SHALL retain the resource and emit structured ownership-uncertain
evidence rather than remove by PID, name, or stale filename.

#### Scenario: Step exceeds the lease TTL

- **WHEN** a step remains active longer than the initial lease TTL
- **THEN** the lease is renewed during the complete resource lifecycle
- **AND THEN** cleanup remains authorized only for the same immutable identity

#### Scenario: Platform identity is unavailable

- **WHEN** the host cannot prove the runtime identity or a renewal is ambiguous
- **THEN** destructive cleanup is skipped
- **AND THEN** a bounded structured failure records the uncertainty

### Requirement: Machine failures use typed contracts

Runtime failures SHALL use a stable `FailureCode` registry and typed retry
classification. Configuration diagnostics SHALL expose structured code,
severity, path, safe message, repair help, and optional source/related
locations. Machine behavior SHALL NOT infer a code or retry category by
matching human display text.

#### Scenario: Display wording changes

- **WHEN** a human-facing message is revised without changing the underlying
  failure
- **THEN** serialized failure code and retry classification remain unchanged
- **AND THEN** scheduler and retry behavior remain unchanged

#### Scenario: Unknown producer code arrives

- **WHEN** a machine consumer receives a failure code it does not recognize
- **THEN** the contract boundary fails closed with bounded evidence
- **AND THEN** the human renderer may show a safe unknown-code diagnostic

### Requirement: Performance changes preserve execution semantics

Task waiting SHALL use a platform wait primitive or a bounded backoff when
polling is required. Scheduler readiness SHALL use deterministic indexes or
remaining-dependency state rather than repeated full scans. The implementation
SHALL preserve dependency ordering, cancellation, failure propagation, cleanup,
and evidence-completeness behavior.

#### Scenario: Fast command completes

- **WHEN** a short-lived process exits before the maximum wait interval
- **THEN** the runner observes completion without an unnecessary fixed delay
- **AND THEN** the result and evidence are identical to the compatibility path

#### Scenario: Wide DAG becomes ready

- **WHEN** a node completes in a workflow with many dependents
- **THEN** only the affected indexed dependents are reconsidered
- **AND THEN** deterministic ordering and resource-preflight exclusions remain

### Requirement: Configuration contracts are maintainable and closed

Validation SHALL avoid cloning the complete configuration for each local
check. Fields documented as closed vocabularies SHALL reject unknown values
through typed deserialization. Environment variables SHALL use the reviewed
`HARNESS_GATE_*` namespace, with any alias migration explicitly documented.
Release-profile panic behavior and runtime unwind boundaries SHALL be tested
and documented.

#### Scenario: Unknown closed vocabulary is supplied

- **WHEN** a configuration uses a value outside a declared closed enum
- **THEN** `config check` fails before external work
- **AND THEN** the diagnostic identifies the field without resolving secrets

#### Scenario: Legacy environment alias is used

- **WHEN** a deprecated environment alias is present during the migration
  window
- **THEN** the behavior follows the documented compatibility rule
- **AND THEN** a safe diagnostic identifies the replacement namespace

### Requirement: Repository and community metadata are governed

The repository SHALL not track Python cache artifacts or undocumented
one-off generated files. It SHALL publish a security policy, MSRV, Issue/PR
templates, and quality-script lint/test coverage. Secret-scan documentation
SHALL describe actual content scanning and its limitations.

#### Scenario: Quality policy is changed

- **WHEN** a quality script or policy check is modified
- **THEN** the script itself is covered by the repository's lint/test and docs
  consistency checks
- **AND THEN** the policy cannot silently diverge from the behavior it reports

## Implementation Plan (proposed)

| Phase | Scope | Target evidence |
| --- | --- | --- |
| 1 | Approve R-07 wording and R-13 egress contract | ADR review, address matrix, redaction cases |
| 2 | Implement R-13 and R-14 independently | Denied-destination, rebinding, heartbeat, identity, and cleanup fixtures |
| 3 | Implement R-15 typed failures and diagnostics | Stable registry, migration snapshots, no display-string branches |
| 4 | Implement R-16 performance changes | Before/after benchmarks and semantic regression tests |
| 5 | Implement R-17 and R-18 maintenance contracts | Config, profile, hygiene, metadata, and CI evidence |
| 6 | Cross-platform verification and closeout | Locked tests, formatter, Clippy, audit, docs checks, and applicable matrix |

Phases are sequencing guidance, not an implementation claim or a release
commitment. Each implementation task remains unchecked until its evidence is
reviewed.

## Technical Examples (illustrative)

The following examples describe intended shapes only; they do not add runtime
syntax or Rust types in this proposal.

```rust
enum FailureCode {
    WebhookDestinationDenied,
    LeaseOwnershipUncertain,
    SchedulerFailure,
}

struct Diagnostic {
    code: FailureCode,
    retry: RetryClass,
    path: Option<String>,
    message: String,
}
```

```toml
[webhook]
url = "https://hooks.example.test/events"
allowed_hosts = ["hooks.example.test"]
```

## Alternatives Considered

- A static private-IP string denylist does not address DNS rebinding or
  connection-time resolution.
- PID, process name, or lease filename matching does not prove ownership after
  reuse or restart.
- Display-message parsing is not a stable machine contract.
- An unmeasured polling rewrite risks semantic regressions without proving
  improvement.
- Calling process groups an OS sandbox would overstate the current guarantee.

## Rollback Plan

Each implementation track SHALL be revertible through a reviewed pull request
and SHALL retain failure evidence. Rollback SHALL not restore unsafe webhook
connections, ambiguous resource deletion, unknown-code passes, or unsupported
OS-sandbox claims. This proposal itself can be reverted without runtime impact
because it does not change the production or staging authority contract.
