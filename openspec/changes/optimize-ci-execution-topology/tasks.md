# Tasks: Optimize CI Execution Topology

All tasks inherit `docs/engineering-policy.md`. CRAP semantics and required assurance remain unchanged throughout this change.

Stage evidence below is historical: pending statements reflect each submission.
The [GH-170 closure record](../../../docs/quality/ci-topology/closure.md) records
the final GH-169 rollback commit's successful hosted CI and the remaining
controller-owned submitted-SHA acceptance boundary. The
[operating guide](../../../docs/quality/ci-topology/README.md) describes the final
model; compiled caching and docs build-once execution were reverted in task 6.

## 1. Freeze current assurance and hosted cost baseline

- [x] 1.1 Record the current PR/push event-to-job matrix, `Required Quality Aggregate` dependencies, stable check names, and every currently required semantic outcome. Explicitly record that macOS and Windows full tests remain PR-required in this change.
- [x] 1.2 Capture normalized hosted timing/cost evidence from representative successful pre-change PR runs: workflow/job/step wall times, critical path, tool-install/setup time, quality collection time, and approximate runner wall minutes by OS.
- [x] 1.3 Add regression fixtures/tests for aggregate event semantics so missing/failed/cancelled/skipped required children still fail closed before topology changes begin.

Task 1 evidence: [frozen assurance contract and hosted baseline](../../../docs/quality/ci-topology/baseline.md), [validation record](../../../docs/quality/ci-topology/validation.md), and independent aggregate/topology fixtures. Native macOS/Windows tests remain PR-required; the stale aggregate classification was repaired without execution optimization. Submitted-commit hosted acceptance is owned by controller CI; subsequent tasks are tracked below.

## 2. Normalize pinned CI tool setup

- [x] 2.1 Replace repeated forced source installs of cargo-nextest/cargo-llvm-cov where appropriate with explicit pinned prebuilt installation or validated tool caches; retain effective version evidence and fail-closed installation behavior.
- [x] 2.2 Replace repeated cargo-audit source compilation with an explicit pinned/prebuilt or validated cached installation while preserving `cargo audit --deny warnings` semantics.
- [x] 2.3 Centralize repeated setup in transparent reusable CI primitives where useful; keep commands, versions, and failure diagnostics observable.

Task 2 implementation evidence: [GH-165 setup and validation record](../../../docs/quality/ci-topology/tool-setup.md) and [checksummed Linux prebuilt smoke results](../../../docs/quality/ci-topology/tool-setup-smoke.json). Four setup regression tests pass, including invalid/missing/failed version evidence. Hosted setup reduction and submitted-commit Required Quality Aggregate acceptance remain controller-owned and pending; tasks 3–7 were not part of that delivery.

## 3. Normalize Cargo cache and build-state boundaries

- [x] 3.1 Define explicit CI Cargo target/cache conventions per OS/job class; verify caches point at the target directory Cargo actually uses and include sufficient invalidation identity.
- [x] 3.2 Separate immutable reusable artifacts from mutable compilation caches. Add fail-closed identity/hash validation for any artifact that participates in authoritative quality/contract consumption.
- [x] 3.3 Evaluate Linux build reuse for compatible consumers and adopt only cases that preserve the existing contract; do not reuse instrumented coverage builds for release/performance claims or replace native cross-platform execution.

Task 3 implementation evidence: [GH-166 Cargo and artifact boundaries](../../../docs/quality/ci-topology/cargo-artifacts.md) and [local validation record](../../../docs/quality/ci-topology/cargo-artifacts-validation.json). Eleven regression tests cover cache identity/path mismatches, stale executable handling and fail-closed artifact transport. The real redirected-target CLI contract run passes. Linux binary reuse was evaluated and no new sharing adopted across incompatible profiles. Hosted Required Quality Aggregate acceptance remains controller-owned and pending; tasks 4–7 and overall proposal acceptance remain open.

## 4. Remove avoidable repeated work

- [x] 4.1 Refactor documentation/preset consistency validation so all current presets, migration behavior, schema sync, policy anchors, and link/sandbox checks remain covered without unnecessary repeated Cargo startup/compilation.
- [x] 4.2 Audit quality-coverage and downstream generic/reference consumers; ensure provenance-sensitive coverage/risk/CRAP/critical-path evidence is collected once per series and reused from retained immutable artifacts rather than recollected.
- [x] 4.3 Audit Linux test/build/clippy/contracts execution for duplicate compilation. Apply only hosted-evidence-backed changes that reduce cost without serializing the PR critical path into a slower monolith.

Task 4 implementation evidence: [GH-167 repeated-work audit](../../../docs/quality/ci-topology/repeated-work.md), [hosted audit input](../../../docs/quality/ci-topology/repeated-work-hosted-audit.json), and [local validation record](../../../docs/quality/ci-topology/repeated-work-validation.json). Real docs reports match all 11 semantic fields; 332 Rust tests and 322 Python tests pass, including all docs failure injections and retained-evidence ownership/replay/transport checks. Formatting, Clippy and strict OpenSpec validation pass. Linux merges and cross-job executable sharing were rejected without hosted after-state evidence. Submitted-commit Required Quality Aggregate acceptance remains controller-owned and pending; tasks 5–7 and overall proposal acceptance remain open.

## 5. Keep Required Quality Aggregate minimal and stable

- [x] 5.1 Preserve the exact `Required Quality Aggregate` check name, `always()` behavior, and event-specific required-child semantics.
- [x] 5.2 Keep the aggregate job limited to fail-closed evaluation of child results and small result emission; prohibit product compilation, tests, coverage/risk collection, or heavy tool installation in the aggregate.
- [x] 5.3 Add/retain tests proving skipped push-only children are handled according to event policy while every PR-required child—including macOS/Windows tests—must succeed.

Task 5 implementation evidence: [GH-168 aggregate contract](../../../docs/quality/ci-topology/aggregate.md) and [local validation record](../../../docs/quality/ci-topology/aggregate-validation.json). All seven aggregate topology fixtures pass, including every frozen required child on both events, push-only exemptions, malformed results, native matrix/CLI failures, exact step allowlist and process/collection-free evaluation. Submitted-commit hosted Required Quality Aggregate acceptance remains controller-owned and pending; tasks 6–7 and overall proposal acceptance remain open.

## 6. Hosted after-state performance and assurance acceptance

- [x] 6.1 Capture multiple representative successful hosted post-change PR runs using the same normalization as task 1.2.
- [x] 6.2 Compare before/after critical path, per-job timing, setup/tool-install time, runner wall minutes by OS, compilation duplication, quality collection time, and artifact overhead. Do not claim a percentage improvement unsupported by hosted evidence.
- [x] 6.3 Run semantic parity review: all prior required outcomes, CRAP/coverage/critical-path semantics, platform requirements, release authority, measurement identities, and fail-closed aggregate behavior remain unchanged.
- [x] 6.4 Revert individual optimizations that are neutral/worse or introduce trust ambiguity rather than weakening assurance to preserve them.

Evidence: [GH-169 hosted comparison, parity review and individual rollbacks](../../../docs/quality/ci-topology/after-state.md);
[exact local validation](../../../docs/quality/ci-topology/after-validation.json).
Both sampled aggregates are green. Submitted-SHA CI and overall closure remain pending.

## 7. Documentation and closure

- [x] 7.1 Document the optimized CI execution model, cache/artifact trust boundaries, pinned tool versions, and how to diagnose cache/artifact failures.
- [x] 7.2 Document the hosted performance comparison and remaining bottlenecks, separating developer critical path from total runner cost.
- [x] 7.3 Identify any future risk-driven conditional cross-platform proposal as a separate Engineering Policy delta; do not implement it here.
- [ ] 7.4 Strict-validate this OpenSpec, run all repository required checks, and record final acceptance/rollback evidence before closure.

Tasks 7.1–7.3: [current operating guide](../../../docs/quality/ci-topology/README.md),
reviewed against workflow/actions, the hosted comparison and task 6 rollbacks.
Task 7.4 local checks and retained required-CI/rollback evidence are recorded in
[closure.md](../../../docs/quality/ci-topology/closure.md) and
[closure-validation.json](../../../docs/quality/ci-topology/closure-validation.json).
Its checkbox remains open until the controller accepts GH-170's final submitted
SHA with all event-applicable Required Quality Aggregate checks green. Historical
green CI and local tests do not substitute for that result. Overall proposal
acceptance and archival remain pending; no product workflow integration is added.
