# ADR-0032: Define Harness-Gate Capability Contracts and the DevRail Boundary

## Status

**Proposed** (2026-08-30)

## Context

The capability assessment of Harness-Gate v0.3.3 (commit
`95678d93ac0815e37e5ab52f3f6a84c97729f85c`) shows that the project is a useful
execution and evidence foundation, but is not yet a drop-in replacement for
the DevRail quality-gate business flow. It already provides project-scoped
configuration, scope selection, dependency-aware execution, basic service
lifecycle management, and JSON/Markdown/JUnit reports. Those features are
currently shaped around one process and one invocation.

The assessment identifies production-blocking gaps when calls are concurrent,
retried, interrupted, or audited after publication: test-runner and isolation
semantics are implicit; report and log paths can be shared; resources have no
cross-process lease; result fields have no stable public schema; and release
assets lack checksums, SBOM, signatures, and provenance. Waivers and standard
retry/flaky/sharding semantics are also missing. Conversely, DevRail owns
supervision, authorization and data scope, redaction, RBAC, notification
outbox, LLM review/repair, organizational audit, and GitHub required-check
integration. Moving those responsibilities into a reusable CLI would create a
larger and less separable product rather than close a Harness-Gate contract.

The consolidated source assessment is recorded in
`docs/code-remediation-2026-08-31-final.md`.

## Decision

1. **Position Harness-Gate as an execution and evidence substrate.** It owns
   configuration validation, scope, DAG scheduling, runner/adapters, managed
   test resources, machine-readable results, and release-verifiable artifacts.
   It does not own DevRail's business control plane or organization policy.
2. **Make the P0 contracts a prerequisite for replacing a DevRail required
   gate.** The first production cutover must have all of the following as
   versioned, tested contracts:
   - runner arguments, environment snapshots, and explicit shared/schema/
     database isolation;
   - invocation-scoped reports, logs, artifact manifests, SHA-256 digests, and
     redaction/retention boundaries;
   - cross-process resource identity, leases, heartbeats, and owner recovery;
   - one result schema with stable IDs, status/error semantics, and evidence
     integrity; and
   - checksums, SBOM, signatures, provenance, and consumer-side verification.
3. **Complete P1 contracts before the first unrestricted production rollout.**
   Waivers must be auditable and expiring, and test results must define
   standard input, retry, flaky, sharding, and merge semantics. A short-lived
   adapter may bridge a missing P1 field only when it emits the same evidence
   and has an explicit expiry and owner.
4. **Treat the P2 adapter protocol as a separate evolution track.** A
   versioned, preferably out-of-process adapter protocol may support new
   runners and scanners later. It is not a reason to add an unstable plugin
   API to the first replacement release.
5. **Adopt a compatibility launcher and staged rollout.** Freeze the current
   DevRail baseline, run a compatibility launcher, compare shadow results,
   promote one canary slice, and retain a documented rollback to the existing
   DevRail path. Parallel execution is opt-in and never substitutes for an
   isolation declaration.
6. **Fail closed at contract boundaries.** Unknown or incompatible contract
   versions, missing isolation for concurrent tests, invalid leases, incomplete
   evidence, failed report writes, and unverifiable release assets are failures
   rather than implicit passes or silent downgrades.

### Ownership boundary

| Responsibility | Harness-Gate | DevRail or external platform |
| --- | --- | --- |
| Config, scope, DAG, runner invocation | Owns | Selects policy and inputs |
| Test isolation, leases, cleanup evidence | Owns | Supplies environment/resource policy |
| Result schema, artifacts, checksums | Owns | Consumes and stores events |
| Waiver approval and organization policy | Emits explicit `WAIVED` state | Owns approval, RBAC, audit, and expiry policy |
| Supervisor, notifications, LLM review/repair | Does not own | Owns |
| Release signing and provenance verification | Produces/validates execution evidence | Owns publication policy and trust roots |
| GitHub required checks and deployment decision | Reports machine result | Owns |

## Consequences

- A DevRail replacement becomes an explicit contract migration instead of a
  command-line substitution, reducing the chance of untraceable or unsafe
  passes.
- Harness-Gate gains durable invocation and evidence boundaries that also help
  standalone CI users, at the cost of storage, cleanup, and schema-version
  maintenance.
- DevRail retains business-specific controls and can evolve them without
  coupling the reusable Rust binary to organization services.
- Initial delivery is intentionally staged. The P2 adapter protocol remains a
  follow-up and cannot be used to bypass P0/P1 acceptance evidence.
- Existing serial configurations remain compatible. Concurrent configurations
  without an isolation declaration fail validation until they are migrated.
- Release operations require signing keys or an equivalent trust service and a
  verification path in clean environments; missing trust material blocks a
  release rather than weakening verification.

## Alternatives Considered

- **Replace DevRail with the current CLI immediately:** rejected because the
  current report, resource, and publication contracts cannot correlate retries
  or prove an artifact's origin.
- **Move DevRail's supervisor, RBAC, and LLM workflows into Harness-Gate:**
  rejected because those are business control-plane responsibilities and would
  make the reusable executor organization-specific.
- **Infer isolation and retry behavior from command strings:** rejected because
  it is not deterministic, cannot be validated before side effects, and is
  impossible to audit reliably.
- **Add an in-process dynamic plugin API first:** rejected because it expands
  the trusted computing base and conflicts with the private-binary boundary in
  ADR-0031. An out-of-process, signed protocol can be evaluated separately.
- **Run a long-lived dual-write migration without a rollback gate:** rejected
  because divergent evidence would be difficult to diagnose and could leave
  resources or required checks in an ambiguous state.

## Rollout and Verification

The OpenSpec change linked below defines the contract schemas, implementation
tasks, compatibility launcher, shadow/canary evidence, and rollback procedure.
The invocation manifest, redaction, retention, checksum, SBOM, signature, and
provenance implementation is covered by the green branch CI
[run 33348751556](https://github.com/musutrade/Harness-Gate/actions/runs/33348751556).
The first operational tag release supplied the previously pending
clean-environment release evidence: immutable `v0.3.5` at commit
`190cfa85699231591e3f74612e38156f6a102ef9`, release workflow run
[33525736285](https://github.com/musutrade/Harness-Gate/actions/runs/33525736285),
and the published [GitHub Release](https://github.com/musutrade/Harness-Gate/releases/tag/v0.3.5)
and [crate](https://crates.io/crates/harness-gate/0.3.5) were verified.
This ADR remains **Proposed** until the P0 contracts are implemented and the
first real DevRail shadow/canary has green cross-platform and evidence-integrity
checks; release-integrity evidence is now complete but does not substitute for
that migration evidence.

## Related

- [OpenSpec: Harness-Gate and DevRail capability contracts](../../openspec/changes/harness-gate-devrail-capability-contracts/proposal.md)
- [Machine-result JSON Schema](../../schema/machine-result.schema.json)
- [Artifact manifest JSON Schema](../../schema/artifact-manifest.schema.json)
- [ADR-0031: Harden Gate Boundaries and Delivery Contracts](0031-harden-gate-boundaries.md)
- [ADR-0025: Phase 1 Quality Baseline Gates](0025-phase-1-quality-baseline-gates.md)
- [ADR-0028: Dependency-Aware Parallel Scheduling](0028-parallel-scheduling.md)
- Consolidated capability and remediation assessment: `docs/code-remediation-2026-08-31-final.md`
