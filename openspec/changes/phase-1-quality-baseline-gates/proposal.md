# Proposal: Establish Phase 1 Quality Baseline Gates

**Status:** Implemented
**Date:** 2026-08-28
**Change type:** Specification and delivery contract only

## Scope Notice

This OpenSpec change defines and implements quality evidence orchestration. It
does **not** implement business behavior or change the product CLI contract.
The Python runners, test-only Rust assertions, fixtures, snapshots, and CI jobs
are delivery mechanisms around the existing binary and remain subject to the
evidence contracts below.

## Why

Harness-Gate is preparing for changes to configuration, verification,
process, service, reporting, and scheduling boundaries. Existing tests,
Codecov reporting, schema generation, cross-platform CI, and historical
benchmarks provide useful signals, but they do not yet form one reproducible
and reviewable quality gate.

The current evidence does not consistently answer these questions:

- Does every core module retain at least 80% line coverage?
- Are cancellation, timeout, cleanup, failure, and report-integrity paths
  directly exercised and traceable, rather than merely incidentally covered?
- Are verification duration, individual step duration, test duration, and the
  shipped binary size measured under a comparable protocol?
- Can a caller rely on CLI output, exit status, error codes, report paths, and
  serialized reports remaining compatible?
- Do documentation examples, local links, and the committed JSON Schema still
  describe and load the same configuration model?

Without explicit answers, a structural refactor can pass happy-path tests while
breaking cancellation cleanup, error classification, report consumers, or
developer workflows. One-off performance measurements are also not comparable
when cache state, toolchain, target, or fixture differs.

## What Changes

- Adds a reviewed OpenSpec contract for coverage, critical-path evidence,
  performance and binary baselines, CLI compatibility, and documentation/schema
  consistency.
- Defines the future evidence package, artifact fields, thresholds, comparison
  rules, platform applicability, and baseline update governance.
- Provides the implementation plan and bounded task list for this delivery;
  the accepted `main` baseline and required-check governance are recorded below.
- Records no capability delta and makes no runtime, configuration, report, or
  CLI change.

## Goals

1. Define a versioned evidence package that can be reproduced locally and in CI.
2. Require at least 80.0% line coverage for each named core module and for the
   aggregate of those modules.
3. Require at least 95.0% passing and traceable evidence for applicable
   cancellation and failure-path scenarios, with no missing evidence for
   cancellation, process-tree cleanup, or report integrity.
4. Establish comparable measurements for total verification time, per-step
   time, test-suite time, and release-small binary size.
5. Freeze the public CLI contract through normalized golden snapshots and
   cross-platform structured assertions.
6. Make local documentation links, example configuration loading, migration
   examples, schema generation, and schema synchronization blocking checks.
7. Define a controlled update process so baselines evolve through reviewed
   evidence instead of silent drift.

## Non-goals

- No production Rust implementation or refactoring.
- No change to CLI arguments, output text, reports, error codes, configuration
  semantics, scheduling, cancellation policy, or service behavior.
- No new user-facing feature, plugin API, container runtime, notification
  channel, or report format.
- No claim that 80% coverage implies complete behavioral safety; the critical
  path matrix remains mandatory.
- No replacement of Codecov as a trend dashboard; local, versioned artifacts
  are the merge-decision source of truth.
- No performance optimization or target reduction in this phase; Phase 1
  records a baseline and detects regressions.
- No unreviewed automatic acceptance of snapshots or baseline changes.

## Proposed Change Boundary

The delivery described by this proposal is limited to quality evidence and gate
orchestration around the existing binary. The expected artifacts and interfaces
are specified in `design.md`; no product capability is added.

The implementation adds a quality manifest, coverage summary, critical-path
matrix, benchmark runner, snapshot fixtures, documentation checker, and CI
jobs. These are delivery mechanisms, not new application capabilities, and
preserve the existing runtime contract.

## Success Metrics

The change is successful only when a green required-check run provides all of
the following reviewable evidence:

| Area | Success criterion |
| --- | --- |
| Core coverage | Every applicable module in the declared boundary is at least 80.0% covered, and the aggregate is at least 80.0%. |
| Critical paths | At least 95.0% of applicable matrix rows are passing and traceable to an executed test and source coverage; cancellation, cleanup, and report-integrity rows are all present and passing. |
| Performance | Five warm verification samples, one cold and five warm test samples, and exact release-small size are recorded with environment metadata. |
| Compatibility | Linux golden snapshots and structured Linux/macOS/Windows contract checks pass for the declared CLI scenarios. |
| Documentation | Local links, fragments, examples, migrations, schema generation, committed-schema diff, and schema validation all pass. |
| Reproducibility | The same documented local commands produce the same artifact schema and can distinguish a valid regression from a new target/toolchain series. |
| Governance | Exceptions identify an issue, owner, expiry, and approval; no threshold is silently lowered. |

## Threshold and Gate Policy

- Coverage and critical-path thresholds are blocking pull-request checks.
- A median increase above 15% in verification time, test time, or binary bytes
  is a blocking regression for a matching target and harness version.
- A changed target triple, toolchain series, or benchmark-harness version starts
  a new baseline series and is not compared numerically to an incompatible
  series.
- CI uploads raw and summary artifacts even when a gate fails.
- An exception does not make a result pass; it records why the gate is
  temporarily accepted and when it must be revisited.

## Implementation And Governance Record

The Phase 1 evidence package was accepted on `main` through the green
[CI run 33466540997](https://github.com/musutrade/Harness-Gate/actions/runs/33466540997)
for commit `6e17867cf42139a3ecd73d014dc566d21d08d562`. The organization ruleset
[`21098892`](https://github.com/musutrade/Harness-Gate/rules/21098892) is active,
targets the default branch with `~DEFAULT_BRANCH`, requires the
`Required Quality Aggregate` check, and sets
`strict_required_status_checks_policy: true`. The effective rules for `main`
are exposed by the [rules-for-main API](https://api.github.com/repos/musutrade/Harness-Gate/rules/branches/main).

The implementation and required-check governance are complete. A deliberately
failing pull request is an optional operational exercise, not an unfulfilled
acceptance criterion for this change.

## Impact and Risk Assessment

**Risk: Medium.** The requested change does not alter runtime behavior, but it
introduces strict contracts that may expose previously untested paths and
incorrect assumptions in documentation or output consumers.

| Dimension | Impact | Control |
| --- | --- | --- |
| Performance | CI consumes additional CPU and artifact storage; benchmark values are hardware-sensitive. | Use fixed fixtures, record environment metadata, compare only compatible series, and retain raw samples. |
| Security | Failure/cleanup evidence reduces the chance of unsafe process or service leftovers; documentation checks prevent invalid operational guidance. | Require actual boundary tests, cleanup assertions, bounded external-link checks, and no secret material in snapshots or artifacts. |
| Maintainability | Snapshot, matrix, and example inventories require deliberate updates when public contracts change. | Version manifests, require reviewed diffs, and assign owners and expiry dates to exceptions. |
| Developer experience | Local verification becomes more predictable but may require installing coverage and benchmark tools. | Publish the exact local commands and keep CI and local artifact formats identical. |

## Dependencies and Assumptions

- The existing Rust CLI remains the system under test.
- The primary production platform is Linux; macOS and Windows remain supported
  development/CI platforms.
- Existing report names and error-code conventions are compatibility inputs,
  including `ERROR [E####]` and the current report file names.
- The implementation team can pin or record versions for Rust, Cargo, coverage,
  test, and benchmark tools.
- A deterministic benchmark workflow can run without a network or Docker
  daemon.

## Related Records

- [ADR-0025: Establish Phase 1 Quality Baseline Gates](../../../docs/adr/0025-phase-1-quality-baseline-gates.md)
- [Review follow-ups](../../../docs/review-followups-2026-08-31.md)
- [ADR-0003: Enhance CI pipeline](../../../docs/adr/0003-enhance-ci-pipeline.md)
- [ADR-0004: Add integration tests](../../../docs/adr/0004-add-integration-tests.md)
- [ADR-0023: Generate configuration schema](../../../docs/adr/0023-config-schema-and-interpolation.md)
- [Phase 2 baseline metrics](../../../docs/benchmarks/phase-2-baseline.md)
