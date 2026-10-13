# Proposal: Harness-Gate and DevRail Capability Contracts

**Status:** Proposed
**Date:** 2026-08-30
**Change type:** Execution contracts, evidence integrity, and integration architecture

## Scope Notice

This change turns the v0.3.3 capability assessment into an implementable
contract and migration plan. It does not claim that the missing P0/P1/P2
capabilities are already implemented. The existing CLI remains supported while
the contracts are delivered behind compatibility and shadow/canary controls.

## Goals

- Define an explicit boundary between Harness-Gate's reusable execution/evidence
  responsibilities and DevRail's business control plane.
- Make every invocation, step, resource, report, and release artifact
  independently identifiable, isolated, and auditable.
- Specify test-runner arguments, environment snapshots, and isolation modes so
  concurrency cannot silently change test behavior.
- Publish a versioned machine-result protocol that DevRail can consume without
  parsing human-readable text.
- Make managed resources recoverable across processes and crashes through
  leases, heartbeats, owner markers, and bounded cleanup.
- Establish release checksum, SBOM, signature, provenance, and verification
  requirements.
- Define auditable waivers and standard retry/flaky/sharding semantics before
  unrestricted production rollout.
- Leave a versioned out-of-process adapter boundary for a later ecosystem track.

## Non-goals

- Do not move Supervisor, RBAC, authorization/data scope, redaction, outbox,
  LLM reviewer/repair, organizational audit, or GitHub required-check policy
  into Harness-Gate.
- Do not introduce a public in-process plugin API in this change.
- Do not change the existing serial default, built-in gate ordering, or current
  CLI report names until compatibility evidence and a separate migration decision
  approve it.
- Do not infer isolation, retry, or waiver policy from arbitrary command strings
  or undeclared environment variables.
- Do not make P2 adapter support a prerequisite for the first built-in DevRail
  cutover.

## Success Metrics

| Area | Success criterion |
| --- | --- |
| Runner contract | DevRail test configurations pass config check; effective args, env snapshot, result format, and isolation mode are present in machine results. |
| Invocation evidence | Two concurrent invocations never overwrite reports, logs, manifests, or raw output; failed atomic writes fail the invocation. |
| Resource safety | Cross-process conflicts are diagnosable; an owner killed mid-run is reclaimable within the lease bound; cleanup failures remain blocking evidence. |
| Result compatibility | A versioned schema and contract tests let DevRail map results to events without parsing Markdown or log text. |
| Supply chain | A clean environment detects any modified binary, checksum, SBOM, signature, or provenance record. |
| Governance | Expired/out-of-scope waivers fail closed; `PASS`, `WAIVED`, and `FAIL` remain machine-distinct. |
| Test semantics | Retry, flaky, sharding, and merge results are replayable and do not hide infrastructure or security failures. |
| Migration | Shadow and canary runs produce equivalent normalized results and a tested rollback path before required-check ownership moves. |

## Risk Assessment

**Risk: High.** These contracts sit at the execution, resource, publication,
and release boundaries. A defect could create a false pass, lose evidence,
leak a service, or publish an unverifiable artifact.

| Risk | Control |
| --- | --- |
| Incompatible test behavior | Versioned runner fields, explicit isolation validation, and captured effective command/environment. |
| Evidence overwrite or leakage | Invocation directories, normalized path allocation, atomic writes, retention rules, and DevRail redaction before export. |
| Orphaned resources | Cross-process leases with owner identity, heartbeat expiry, dry-run cleanup, and platform lifecycle tests. |
| Schema drift | `schema_version`, compatibility policy, fixtures, and consumer contract tests. |
| Supply-chain compromise | Immutable release assets, checksums, SBOM, signed provenance, pinned toolchain/actions, and offline verification. |
| Migration regression | Frozen baseline, shadow comparison, one canary slice, required-check holdback, and one-command rollback. |

## Delivery Sequence

1. Freeze the v0.3.3 DevRail baseline and capture representative serial evidence.
2. Add the compatibility launcher and invocation/evidence directory model.
3. Implement P0 runner, lease, result, and release contracts with cross-platform
   contract tests.
4. Implement P1 waiver and standard test-result semantics, or document a
   time-bounded adapter with an owner and expiry.
5. Run shadow and canary slices; compare normalized machine results and cleanup
   evidence while DevRail remains the required-check owner.
6. Transfer ownership gradually, keeping rollback available until the canary
   retention window and post-run audit complete.
7. Design and implement the P2 adapter protocol as a separate proposed change.

## Related Records

- [ADR-0032: Define Harness-Gate capability contracts and the DevRail boundary](../../../docs/adr/0032-harness-gate-devrail-capability-contracts.md)
- [ADR-0031: Harden Gate Boundaries and Delivery Contracts](../../../docs/adr/0031-harden-gate-boundaries.md)
- Consolidated capability and remediation assessment: `docs/code-remediation-2026-08-31-final.md`
- [ADR-0025: Phase 1 Quality Baseline Gates](../../../docs/adr/0025-phase-1-quality-baseline-gates.md)
- [ADR-0028: Dependency-Aware Parallel Scheduling](../../../docs/adr/0028-parallel-scheduling.md)
