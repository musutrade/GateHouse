# Proposal: Unify Built-in Gates and Configured Steps in a Verification Plan

**Status:** Implemented
**Date:** 2026-08-28
**Change type:** Execution architecture and compatibility contract

## Scope Notice

This OpenSpec defines and implements the Phase 3 execution-plan contract. The
implementation boundary is limited to the private plan, typed built-in gate
declarations, and compatibility behavior described here; it does not authorize
business workflow changes, a parallel scheduler, a new gate, or a report-schema
migration.

## Why

`depends_on`, dependency closure, and stable topological ordering already exist
for configured external steps. Secret scanning and architecture auditing are
still executed as a hard-coded preamble, so the system has no single model for
node identity, dependencies, results, failure propagation, or cancellation.
The fixed sequence is therefore an undocumented control-flow assumption rather
than a verifiable default graph.

Before later work introduces parallel execution or additional gates, the
existing behavior must be represented explicitly and kept compatible. A
configuration author also needs a reviewed vocabulary for declaring a built-in
gate without falling back to arbitrary commands.

## What Changes

- Define a private `VerificationPlan`, `PlanNode`, and common `NodeResult`
  contract.
- Represent secret scan, architecture audit, and configured external steps as
  nodes with stable IDs and typed kinds.
- Specify a closed `kind = "builtin-gate"` / `gate_type` configuration shape;
  unknown gate types fail closed.
- Synthesize a legacy-compatible `secret -> audit -> external steps` DAG when
  built-in nodes are absent, without rewriting configuration files.
- Define deterministic topological execution, status propagation, report
  adaptation, error-code preservation, and cancellation invariants.
- Define compatibility evidence that compares the default DAG with the current
  fixed orchestration.

## Goals

1. Make all verification work consumable through one internal plan boundary.
2. Preserve legacy ordering, output, report paths, and `E1402`/`E1403`/`E1404`
   behavior for valid existing configurations.
3. Make gate and external-step failures, skipped descendants, and cancellation
   explicit and testable.
4. Prevent undeclared or unknown built-in gate types from executing implicitly.
5. Leave a safe handoff for a future scheduler without implementing parallelism.

## Non-goals

- No parallel execution, worker pool, service locking, or cancellation fan-out.
- No new business gate beyond the existing secret scan and architecture audit.
- No HTML/Tera rendering, report format redesign, or public Rust library API.
- No automatic insertion of `depends_on`, configuration rewrites, retries, or
  changed profile/scope semantics.
- No change to existing CLI output, report names, report fields, or public error
  codes except through the compatibility adapter defined by this change.

## Success Metrics

| Area | Success criterion |
| --- | --- |
| Plan model | Every selected gate and external step has a deterministic node ID, kind, dependencies, and lifecycle status before execution begins. |
| Default DAG | A legacy configuration produces exactly the established secret-scan, architecture-audit, then external-step order, including dependency closure. |
| Configuration | Valid built-in declarations are parsed by a versioned closed vocabulary; missing, duplicate, cyclic, or unknown references fail before side effects. |
| Results | Built-ins and external steps map to one internal result model while existing report shape and labels remain compatible. |
| Failure/cancellation | Gate failures block the documented descendants; cancellation preserves cleanup and `E1402`; no skipped or cancelled node is reported as passed. |
| Compatibility | Golden fixtures show no unintended change to CLI streams, report paths, report shape, ordering, or `E1402`/`E1403`/`E1404`. |
| Scope control | No scheduler, renderer, or business implementation is present in this change. |

## Risk Assessment

**Risk: High.** Verification orchestration is a safety and compatibility
boundary. A mistaken default edge could bypass a mandatory gate, while a
result-model change could break CI consumers or obscure cancellation. Controls
are a closed node vocabulary, synthesized edges kept internal, fail-closed
validation, and fixture-based output/error regression tests before acceptance.

## Dependencies and Assumptions

- ADR-0024 remains the source of truth for dependency closure and stable
  topological ordering.
- ADR-0025 quality contracts remain mandatory evidence for output, reports,
  cancellation, and error codes.
- ADR-0026 resource preflight remains applicable to external-step resources and
  is not relaxed by introducing plan nodes.
- Existing secret and audit adapters remain the owners of their I/O, reports,
  cleanup, and typed failures.
- The default execution policy remains serial until a separate scheduler ADR is
  accepted.

## Related Records

- [ADR-0024: Add Dependency Ordering to Verification Steps](../../../docs/adr/0024-verification-plan-dependencies.md)
- [ADR-0025: Establish Phase 1 Quality Baseline Gates](../../../docs/adr/0025-phase-1-quality-baseline-gates.md)
- [ADR-0026: Add Configuration Safety Diagnostics and Future-Concurrency Preflight](../../../docs/adr/0026-configuration-safety-diagnostics.md)
- [ADR-0027: Unify Built-in Gates and Configured Steps in a Verification Plan](../../../docs/adr/0027-unified-verification-plan.md)
- [Verification-plan dependency foundation](../verification-plan-dependencies/proposal.md)
