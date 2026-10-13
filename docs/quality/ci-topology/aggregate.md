# GH-168: Keep Required Quality Aggregate minimal and stable

This implements `optimize-ci-execution-topology` tasks 5.1–5.3 after GH-167.
[Engineering Policy](../../engineering-policy.md), CRAP semantics, required
assurance, thresholds, measurement series and release authority are unchanged.
[ADR-0039](../../adr/0039-required-risk-and-traceability-gates.md) and
[ADR-0040](../../adr/0040-language-agnostic-evidence-policy.md) remain governing
records; this execution contract needs no new policy decision.

The workflow keeps the exact `Required Quality Aggregate` name, `always()`
condition, dependencies, and event policy from the [frozen contract](baseline.md).
Full macOS and Windows nextest matrix rows remain unconditional and PR-required.
Only cross-platform builds, Codecov coverage, cross-platform contracts, and
performance baselines are push-only. Their results do not affect PR aggregation;
each must succeed on pushes. Missing, failed, cancelled or skipped PR-required
children block both events, even when every other child succeeds.

The job still has only checkout, Python setup, and `ci_quality.py aggregate`.
A regression test compares the complete step content against this allowlist,
preventing extra actions, multiline commands, conditional evaluation, and error
suppression. Product compilation, tests, coverage/risk collection and heavy Rust
tool installation are prohibited. CLI tests forbid process launches, candidate
collection/verification and evidence writes while checking exit status and small
diagnostics. Existing shared collector code remains outside the aggregate path.

Malformed top-level needs payloads now produce a controlled failure. Non-object
required child entries are reported as unsuccessful children rather than raising
an uncaught attribute error. Only the exact result `success` passes. Unsupported
events and malformed JSON remain failures; no replacement evidence is collected.

The original three semantic fixtures passed before editing. The expanded seven
fixtures first exposed 135 malformed-input subcase errors, then passed after the
input guards. They cover all frozen children across both events, explicit push-only
exemptions, unexpected result values, native test failure through the CLI, exact
workflow topology, and the minimal execution boundary. Full local command results
and environment limitations are retained in [the validation record](https://github.com/musutrade/Harness-Gate/blob/d9a932b23dcfea709175285b594ca200450a9971/docs/quality/ci-topology/aggregate-validation.json).

Submitted-commit hosted Required Quality Aggregate acceptance is pending and
controller-owned. No hosted speedup is claimed. Tasks 6–7 and overall OpenSpec
acceptance remain open. Rollback restores these guards and diagnostics to the
prior implementation without changing requiredness, platform execution or policy.
