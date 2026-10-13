# Proposal: Dependency-Aware Parallel Scheduling

**Status:** Implemented
**Date:** 2026-08-28
**Change type:** Execution architecture, configuration contract, and test-plan specification

## Scope Notice

This OpenSpec defines and records the implemented Phase 4 parallel-scheduling
contract. The reviewed implementation adds a private scheduler, execution
configuration, resource coordination, deterministic publication, and focused
evidence without adding a new business gate or changing the serial default.

## Why

ADR-0024 established dependency closure and stable topological ordering, ADR-0026
established static service/log safety preflight, and ADR-0027 unified built-in
gates and external steps as `VerificationPlan` nodes. Execution remains serial,
so independent nodes cannot use available capacity. Adding worker threads
without a scheduler contract would make dependency readiness, service startup,
log ownership, output order, and cancellation depend on timing.

The phase-four work needs one explicit answer for each of these concerns:

- how users opt into parallel execution and cap concurrency;
- how only dependency-ready nodes are dispatched;
- how reusable and exclusive services are shared safely;
- how each node receives a non-colliding log path;
- how results are merged and printed in stable plan order;
- how cancellation, timeout, and failed ancestors affect queued and running
  nodes; and
- how performance and service-contention behavior are measured reproducibly.

## What Changes

- Define a private scheduler boundary consuming `VerificationPlan` and an
  explicit `ExecutionPolicy`.
- Add the optional v2 `[execution]` configuration with `parallel` and
  `max_parallel`, retaining serial behavior by default.
- Define stable ready-queue dispatch for dependency-independent nodes and a
  bounded worker capacity.
- Define service resource identities, shareability, lock ownership, startup
  reuse, and exactly-once cleanup.
- Define unique per-step log allocation and a single ordered result publisher.
- Define one cancellation, timeout, and failure-propagation contract for
  built-in gates and external steps.
- Define compatibility snapshots, contention/stability tests, and serial vs
  parallel performance evidence.

## Goals

1. Enable safe, bounded parallel execution of selected nodes that have no
   dependency path between them.
2. Preserve serial behavior, CLI output order, report paths, report shape, and
   `E1402`/`E1403`/`E1404` mappings for legacy configurations and
   `parallel = false`.
3. Prevent service races, duplicate startup, mutable shared-state races, and
   log-file overwrites before or during execution.
4. Make queued cancellation, running cancellation, timeout, skipped
   descendants, and multi-failure selection deterministic and testable.
5. Produce comparable performance and contention evidence using the baseline
   rules established by ADR-0025.

## Non-goals

- No new business gate, retry policy, distributed scheduler, or public Rust
  scheduler API.
- No relaxation of ADR-0026 static resource-preflight rules or ADR-0027
  mandatory legacy gate edges.
- No completion-order CLI/report output, shared step logs, process-global
  environment mutation, or implicit configuration repair.
- No change to existing report names, report schema, error codes, scope/profile
  semantics, or serial failure behavior without a separate compatibility
  decision.
- No claim that parallel execution always improves wall-clock time; benchmark
  results remain fixture, target, and hardware specific.

## Success Metrics

| Area | Success criterion |
| --- | --- |
| Configuration | Omitted `parallel` remains serial; enabled parallel mode uses a default `max_parallel` of 4, accepts 1–64, and rejects invalid values before side effects. |
| Scheduling | Ready nodes never exceed the effective limit, never start before successful dependencies, and use stable topological/configuration tie-breaking. |
| Resources | Static conflicts fail preflight; runtime locks prevent duplicate startup and unsafe concurrent use; cleanup occurs exactly once for all terminal states. |
| Logs and output | Every external node has a unique contained log path; CLI and reports publish results in stable plan order regardless of completion order. |
| Cancellation/failure | Queued and running cancellation, timeout, failed descendants, unrelated branches, and primary-error selection follow the documented policy and preserve public codes. |
| Performance | Fixed fixtures record serial/parallel wall time, per-step duration, peak concurrency, scheduler overhead, and service-start reuse with ADR-0025 metadata. |
| Stability | Repeated parallel runs show no leaked process, service, lock, or log artifact and produce equivalent normalized output. |

## Impact and Risk Assessment

**Risk: High.** Concurrency changes the timing and resource profile of the
verification boundary. A scheduler defect could bypass dependencies, leak a
service, overwrite evidence, or report a result in a nondeterministic order.

| Dimension | Impact | Control |
| --- | --- | --- |
| Safety | Incorrect readiness or lock ownership could run unsafe work concurrently. | Reuse the validated plan/reachability relation, fail closed on static conflicts, and require runtime lock/cleanup tests. |
| Compatibility | Parallel completion can change visible ordering or failure selection. | Keep serial as the default, publish by stable plan order, and compare legacy snapshots byte-for-byte or field-for-field. |
| Performance | Workers increase CPU, descriptors, memory, and service capacity. | Enforce 1–64 bounds, record peak concurrency and overhead, and compare only compatible baseline series. |
| Cancellation | Fan-out and bounded cleanup may leave partial results. | Use one plan-level token, explicit terminal statuses, bounded waits, and actual process/service cleanup assertions. |
| Maintainability | Scheduler, lock registry, and publisher add coordination code. | Keep adapters task-local, centralize policy, prohibit map-iteration ordering, and require focused module tests. |

## Dependencies and Assumptions

- ADR-0024 remains the source of truth for dependency closure and stable
  topological ordering.
- ADR-0025 remains the source of truth for compatibility snapshots and
  benchmark metadata/regression thresholds.
- ADR-0026 static resource preflight runs before scheduler dispatch and is not
  weakened by runtime locking.
- ADR-0027 `VerificationPlan`, `PlanNode`, `NodeResult`, and synthesized gate
  edges are available as private inputs.
- Existing process cancellation and service adapters remain owners of their
  process-tree and external-resource cleanup operations.
- The implementation can provide a cancellation token and bounded asynchronous
  or equivalent worker coordination without exposing a new public API.

## Related Records

- [ADR-0028: Introduce Dependency-Aware Parallel Scheduling](../../../docs/adr/0028-parallel-scheduling.md)
- [ADR-0024: Add Dependency Ordering to Verification Steps](../../../docs/adr/0024-verification-plan-dependencies.md)
- [ADR-0025: Establish Phase 1 Quality Baseline Gates](../../../docs/adr/0025-phase-1-quality-baseline-gates.md)
- [ADR-0026: Add Configuration Safety Diagnostics and Future-Concurrency Preflight](../../../docs/adr/0026-configuration-safety-diagnostics.md)
- [ADR-0027: Unify Built-in Gates and Configured Steps in a Verification Plan](../../../docs/adr/0027-unified-verification-plan.md)
- [Verification-plan specification](../unified-verification-plan/specs/verification-plan/spec.md)
