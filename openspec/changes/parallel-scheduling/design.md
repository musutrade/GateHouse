# Design: Dependency-Aware Parallel Scheduler

## Scope Boundary

This design is normative for the implemented Phase 4 execution boundary. It
defines the configuration, scheduler state machine, resource ownership,
output, and evidence contracts without prescribing a public Rust API or adding
a new business capability.

## Existing Behavior to Preserve

When `[execution]` is absent or `parallel = false`, verification remains
serial. The synthesized `secret-scan -> architecture-audit -> external` chain
from ADR-0027, configured dependency closure from ADR-0024, existing scope and
profile selection, report paths, CLI streams, and public error codes remain
unchanged.

## 1. Configuration and Effective Policy

The v2 configuration may contain:

```toml
[execution]
parallel = true
max_parallel = 4
```

The effective policy is:

| Input | Effective behavior |
| --- | --- |
| Section omitted | `parallel = false`; serial execution. |
| `parallel = false` | Serial execution. A valid `max_parallel` has no execution effect. |
| `parallel = true`, `max_parallel` omitted | Parallel execution with `max_parallel = 4`. |
| `parallel = true`, `max_parallel = 1..=64` | Parallel policy with the explicit bound. |
| `max_parallel = 0`, negative, or greater than 64 | Configuration error before plan execution. |

The schema and diagnostics must expose this default and range. An explicit
parallel configuration must never be silently downgraded to serial because of
capacity, an invalid value, or a runtime lock; it fails with a typed result
instead. Host CPU count does not alter the configured bound.

## 2. Scheduler Inputs and State

The private scheduler receives:

```text
validated VerificationPlan
effective ExecutionPolicy
plan-level cancellation source
node adapters and task-local execution context factory
ordered report/output publisher
```

It maintains state for each selected node: `pending`, `ready`, `running`,
`passed`, `failed`, `cancelled`, or `skipped`. Dependency counters and reverse
edges are derived from the already validated plan. The scheduler must reject an
inconsistent plan rather than silently drop a node or edge.

## 3. Readiness and Dispatch

A pending node becomes ready only when every dependency has status `passed`.
If any dependency is `failed`, `cancelled`, or `skipped`, the node becomes
`skipped` and cannot run. A node that is not selected is never dispatched and
does not satisfy a selected dependency.

The ready queue is ordered by the plan's stable topological index, then the
original configuration index. The scheduler repeatedly performs this logical
loop:

```text
while pending, ready, or running nodes remain:
    publish deterministic terminal transitions
    mark newly eligible nodes ready
    dispatch the earliest ready nodes until running == effective limit
    await one worker result, cancellation, timeout, or lock event
```

In serial mode the same loop dispatches one node. In parallel mode it may
dispatch multiple ready nodes, but never exceeds `max_parallel`. Worker
completion order affects only internal wakeups; it never determines public
result order or primary error selection.

Legacy synthesized gate edges remain ordinary dependencies. Therefore the
legacy secret scan and architecture audit still serialize before external
steps even when parallel mode is enabled, while independent external branches
may run concurrently after both gates pass.

## 4. Resource Ownership and Locks

ADR-0026 preflight is authoritative for statically knowable conflicts. Each
service adapter additionally declares a stable resource identity and whether a
ready instance is shareable. The runtime lock registry is keyed by that
identity and exposes exclusive and shared acquisition semantics:

- startup and teardown transitions are exclusive;
- a shareable ready instance may serve multiple node contexts concurrently;
- a non-shareable instance is held exclusively for one node;
- lock waits observe cancellation and a bounded wait policy;
- failed startup releases ownership and removes partial state; and
- the final owner performs cleanup exactly once after success, failure,
  timeout, or cancellation.

Node contexts are task-local. Injected service environment variables are passed
to child processes only; no worker writes process-global environment state.
Mutable adapter state must be synchronized behind the adapter boundary.

Runtime locking is defense in depth. It cannot make a statically rejected
configuration valid, add dependencies, rename injection names, or serialize a
duplicate log silently.

## 5. Log Allocation and Ordered Publication

Before dispatch, the scheduler or plan boundary allocates a unique normalized
relative log path for every selected external node. Allocation rejects
normalization collisions and paths outside the existing report-output root.
Each node owns its file; no two nodes append to the same step log.

Workers return `NodeResult` values and do not print final summaries or mutate
the shared report model. A single publisher buffers results as needed and
emits them in stable plan order. It preserves existing labels, report names,
JSON/Markdown fields, ANSI policy, and path formatting. Durations and completion
timestamps may be stored as fields but cannot reorder output.

Report writing occurs after the required results are available and remains
coordinated through the existing report adapter. A report-write error maps to
`E1404`; a result is not presented as passed if publication fails.

## 6. Cancellation, Timeout, and Failure Policy

Cancellation is a plan-level signal observed by dispatch, lock acquisition,
workers, and adapters:

1. once observed, no pending or ready node starts;
2. queued nodes become `cancelled` (or the established compatibility status
   `skipped` where the public report requires it);
3. running external work uses the existing process-tree cancellation path and
   is awaited for bounded cleanup;
4. built-in adapters observe the same token at their owned cancellation
   points; and
5. every affected node is published as non-passing and descendants cannot run.

Timeout is a typed cancellation reason. The timed-out node retains its log and
cleanup observations, descendants are blocked, and its public mapping follows
the existing timeout/execution contract. No implicit retry occurs.

The default parallel failure policy is dependency-local: a failed node blocks
its descendants, while unrelated ready branches may continue until a
plan-level fail-fast policy requests cancellation. Serial mode retains the
captured pre-scheduler continuation behavior. If multiple nodes fail, the
primary public error is selected by stable plan order; other failures remain
attached as structured internal details.

Public mappings remain `E1402` for cancellation, `E1403` for execution
failure, and `E1404` for report-write failure. No internal worker identifier or
completion timestamp is added to legacy output without a separate report
contract.

## 7. Verification Evidence and Rollback

The implementation must provide deterministic unit tests for queue ordering,
limits, dependency propagation, cancellation, lock waiting, and publication.
Integration tests must use controlled barriers or virtual timing to vary
completion order without relying on scheduler luck. Benchmarks follow ADR-0025
and record serial and parallel series separately.

If compatibility or cleanup evidence fails, rollback is one reviewed revert of
the scheduler, execution-policy parsing, lock registry, log allocator, and
publisher integration. Existing configurations continue to parse because the
new section is optional, and the runtime returns to the serial orchestrator.
Rollback must not rewrite configuration files, delete existing reports, or
leave managed services running.

## Alternatives Considered

### Spawn one worker per ready node at call sites

Rejected because readiness, cleanup, cancellation, and output ordering would
be duplicated and timing-dependent.

### Dispatch every selected node and rely only on runtime locks

Rejected because dependency and mandatory-gate failures would occur after
side effects, and locks cannot repair static log or environment conflicts.

### Print results in completion order

Rejected because output, snapshots, and report consumers would become
nondeterministic.

### Duplicate every service for every node

Rejected because it wastes resources and cannot safely model non-shareable
external state.

### Enable parallelism by default

Rejected because it changes resource usage and failure timing for existing
configurations without an explicit migration decision.
