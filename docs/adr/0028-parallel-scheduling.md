# ADR-0028: Introduce Dependency-Aware Parallel Scheduling

## Status

**Accepted** (2026-08-29)

## Context

ADR-0024 established `depends_on`, dependency closure, and deterministic
topological ordering. ADR-0027 unified built-in gates and configured external
steps as `VerificationPlan` nodes with common result and cancellation
semantics. Execution is still serial, so independent nodes cannot use available
CPU or service capacity, and the existing execution boundary does not yet
define how parallel work is made safe or reproducible.

The next execution phase must answer these questions before concurrency is
enabled:

- Which configuration controls opt in to parallel execution, and what is the
  default for existing files?
- Which nodes may run concurrently when dependencies, profiles, scopes, or
  selection rules are present?
- How are service startup, shared state, and resource ownership protected from
  duplicate starts and races?
- How does each step obtain an independent log path, and how are results
  printed in a stable order when completion order is nondeterministic?
- What happens to running and ready nodes after cancellation, timeout, or a
  failure, and which error code and report status are observable?
- How can performance and contention regressions be measured without making
  timing-dependent output part of the public contract?

Implicit answers in worker-thread code would make future gates, retries, and
report consumers depend on scheduling accidents. A scheduler therefore needs
  an explicit contract layered on the plan and safety diagnostics already
  defined by ADR-0026 and ADR-0027.

## Decision

Introduce a private, dependency-aware scheduler that executes a
`VerificationPlan` under an explicit execution policy. This ADR defines the
architecture and compatibility contract for parallel execution; it does not
define a new gate, retry policy, distributed execution, or a public scheduler
API.

### 1. Scheduler boundary and execution policy

Add an internal `scheduler` module owned by verification execution. It accepts
an already validated `VerificationPlan`, an execution policy, cancellation
source, and node adapters. Configuration loading, interpolation, dependency
validation, resource preflight, and report serialization remain outside the
scheduler.

The policy is represented by the optional v2 configuration section:

```toml
[execution]
parallel = true
max_parallel = 4
```

The fields have these semantics:

| Field | Contract |
| --- | --- |
| `parallel` | Opts into concurrent execution of eligible nodes. Omitted or `false` preserves the existing serial execution policy. |
| `max_parallel` | Optional positive upper bound on simultaneously running nodes. When `parallel = true` and it is omitted, the effective value is **4**. Explicit values must be in the inclusive range 1–64. When `parallel = false`, a valid value is accepted for forward-compatible configuration but has no execution effect. |

The schema, diagnostics, and migration guide must state the default of 4 and
reject zero, negative, and values above 64 before execution. The effective
limit is therefore bounded without depending on host CPU count or silently
creating unbounded resources. Existing v2 configurations remain serial without
edits. A future policy field requires a separate ADR rather than overloading
these two fields.

The scheduler must use the stable topological order from ADR-0024/ADR-0027 as
its ready-queue tie breaker. It must not introduce a second dependency
ordering algorithm or use hash/map iteration order.

### 2. Eligibility and constrained parallelism

At any scheduling point, a selected node is *ready* only when all of its
dependencies have completed successfully. A node is not eligible when an
ancestor failed, was cancelled, or was skipped. The scheduler may dispatch up
to `max_parallel` ready nodes when `parallel = true`; with `parallel = false`
it dispatches one node at a time using the same queue.

Readiness is constrained by the following ordered checks:

1. plan selection and dependency closure from the existing plan builder;
2. failure/cancellation state of all ancestors;
3. static resource declarations validated by ADR-0026; and
4. runtime acquisition of shared service/resource locks.

The scheduler must not infer independence from current profile or scope values
unless that relation is encoded in the plan. It must conservatively treat two
nodes as potentially concurrent when neither is reachable from the other.
Static preflight remains authoritative for conflicts that can be identified
from configuration. Runtime locks are defense in depth for shared resources
whose ownership is represented by an adapter.

Built-in gates and external steps use the same dispatch lifecycle and
`NodeResult` model. A built-in gate may run concurrently with another node
only if its plan dependencies and resource declarations make it eligible;
legacy synthesized gate edges from ADR-0027 therefore continue to serialize
`secret-scan -> architecture-audit -> external steps` unless an explicitly
reviewed plan changes those edges.

### 3. Services, shared state, and resource locks

Each node execution receives a task-local execution context. The context may
hold service handles, child-process state, cancellation state, and a log sink,
but it must not mutate process-global environment variables or share mutable
service state without synchronization.

Service adapters expose a stable resource identity. Before starting or using a
service, a node acquires an async-compatible lock for that identity. The lock
contract is:

- at most one startup/teardown transition for a resource is active at a time;
- concurrent users of a reusable ready service share one managed instance when
  the adapter declares the resource shareable;
- non-shareable resources are held exclusively for the node lifetime;
- a failed startup releases the lock and leaves no partially managed resource;
- cleanup runs exactly once after the final user, on success, failure, timeout,
  and cancellation; and
- lock acquisition observes plan cancellation and has a bounded wait, with a
  typed scheduler failure if the bound is exceeded.

The scheduler never resolves a static conflict by changing dependencies,
renaming injected environment variables, or silently serializing an unsafe
configuration. Such conflicts are rejected by the ADR-0026 preflight rules.
Runtime locks cover only adapter-declared identities and cannot be used to
justify an otherwise invalid configuration.

### 4. Independent logs and deterministic result publication

Every external step receives a unique, normalized log path derived from the
step identity and the plan invocation. Built-in gates retain their established
report/log locations. The path allocator must reject collisions before any
node starts, including collisions introduced by normalization, and must keep
all outputs below the existing report directory policy.

Worker completion order is not observable as report order. The main execution
thread (or its single publication owner) collects each `NodeResult` and merges
results according to the plan's stable topological order, with the original
configuration order as the tie breaker already defined by ADR-0024. It prints
CLI progress and final summaries in that same order. A node's duration and
completion timestamp may be recorded as data, but timestamps and worker
interleavings must not reorder or rewrite deterministic output.

Log contents are written by their owning node and are never appended to a
shared step log. Report writing remains a single coordinated operation using
the existing `E1404` mapping for write failures. A report must not claim a
node passed until its result has been published successfully.

### 5. Cancellation, timeout, and failure propagation

Cancellation is a plan-level event observed by queue dispatch, lock waits, and
node adapters:

- no node that has not started may begin after cancellation is observed;
- queued ready nodes become `cancelled` (or the existing report-equivalent
  `skipped` status where compatibility requires it);
- running external tasks receive cancellation through the existing process
  boundary and are awaited for bounded cleanup;
- running built-in gates receive the same cancellation token at their owned
  observation points; and
- no cancelled or skipped node is reported as passed.

Each node may also have a configured or policy-derived timeout. Timeout is a
typed cancellation reason, not a generic dependency failure. The node's log
and cleanup state are retained, descendants are prevented from starting, and
the established timeout/failure mapping is preserved in the public result.

The default failure policy is fail-fast for mandatory prerequisites and
dependency-local for independent branches:

- a failed node prevents all descendants from starting;
- unrelated ready nodes may continue until the first failure is published or a
  plan-level fail-fast cancellation is requested by policy;
- the default policy for existing serial execution remains unchanged; and
- the policy must be explicit in the internal model so a later change cannot
  be inferred from thread behavior.

The final plan result is failed if any selected node failed, timed out, or was
cancelled. Error codes remain compatible with existing boundaries: cancellation
uses `E1402`, execution failures use `E1403`, and report-write failures use
`E1404`. When multiple nodes fail, the primary error is selected by stable plan
order; all other failures remain attached as structured node details.

### 6. Compatibility and migration contract

Serial execution is the compatibility mode. For a legacy configuration or
`parallel = false`, the scheduler must produce the same gate order, step order,
CLI text, report paths, report shape, and error codes as the pre-scheduler
orchestrator, subject only to the reviewed result-model adapter in ADR-0027.

Enabling `parallel` is an opt-in behavioral change. The configuration checker
must reject an invalid `max_parallel`, unsafe resource declaration, duplicate
log, or unsupported policy before execution. It must not silently downgrade an
explicitly enabled parallel configuration to serial; a downgrade would hide a
configuration or capacity problem.

The migration guide must document the serial default, the new `[execution]`
fields, maximum limits, log naming expectations, and the fact that independent
branches can complete in a different wall-clock order while printed output
remains stable. Existing report consumers must continue to rely on stable
topological/configuration order, not completion timestamps.

### 7. Evidence and acceptance gates

The implementation is accepted only when the following evidence is checked in
and run in CI:

1. scheduler unit tests cover ready-queue ordering, dependency closure,
   `max_parallel`, serial compatibility, and cancellation while queued,
   running, or waiting for a lock;
2. integration tests cover two independent steps, a dependency chain, a
   failed branch with an unrelated branch, timeout, service startup reuse,
   non-shareable service exclusion, and cleanup after every terminal state;
3. log tests prove unique per-step paths, no overwrite under parallel runs,
   normalized-path collision rejection, and report-directory containment;
4. output snapshots prove stable CLI/report order across repeated runs with
   intentionally varied completion timing, including stable `E1402`, `E1403`,
   and `E1404` behavior;
5. performance benchmarks compare serial and parallel wall time, per-step
   durations, peak concurrency, scheduler overhead, and service startup reuse
   using fixed fixtures and the baseline recording rules from ADR-0025; and
6. contention/stability tests run repeated parallel invocations and assert no
   leaked process, service, lock, or log artifact.

Benchmark results are evidence, not a promise of a universal speedup. A
regression threshold and accepted hardware/target-series rules follow
ADR-0025. Parallel tests must be deterministic in assertions and may use
controlled barriers or virtual clocks rather than relying on scheduler luck.

## Implementation Evidence

The private scheduler, bounded ready queue, dependency-local blocking,
resource leases, cancellation/timeout handling, ordered publication, and
parallel configuration were implemented in [PR #36](https://github.com/musutrade/Harness-Gate/pull/36).
The cross-platform and quality evidence passed in
[main CI run 33177851104](https://github.com/musutrade/Harness-Gate/actions/runs/33177851104).
The benchmark fixture and comparison rules are recorded in
`docs/benchmarks/parallel-scheduling.md` and the quality baseline artifacts.

## Consequences

### Positive

- Independent verification work can use available capacity while preserving
  dependency correctness and stable user-visible output.
- Service ownership, cleanup, and log isolation become explicit scheduler
  responsibilities instead of incidental thread behavior.
- Cancellation and failure propagation have one plan-level contract that
  future gates and adapters can consume.
- Existing configurations remain serial and compatible until concurrency is
  deliberately enabled.

### Negative

- A scheduler, lock registry, cancellation fan-out, and result publisher add
  coordination complexity to the verification path.
- Resource identities and shareability become adapter contracts that require
  review whenever a new service type is added.
- Parallel execution consumes more CPU, file descriptors, and service capacity;
  conservative limits and preflight checks can reject configurations that
  happened to work serially.
- Deterministic output requires buffering or ordered publication, which can
  delay progress text and increase memory use for large plans.
- Timing-sensitive tests and benchmarks require controlled fixtures and more
  CI evidence than the serial path.

## Alternatives Considered

### Spawn one thread per ready node without a scheduler module

Rejected. It leaves dependency readiness, cancellation, resource ownership,
and output ordering distributed across call sites and makes safety behavior
dependent on worker timing.

### Run all nodes concurrently and rely on runtime locks

Rejected. Dependencies and mandatory gates would be discovered too late, and
runtime locks cannot repair static log or environment conflicts before side
effects occur.

### Preserve completion-order output

Rejected. Completion order is nondeterministic and would break CLI snapshots,
report consumers, and reproducible incident comparison.

### Duplicate every service per parallel node

Rejected. It wastes resources, increases startup time, and does not solve
shared external state or non-shareable service semantics. Adapters must declare
whether a resource can be reused or requires exclusion.

### Make parallel execution the default for new and legacy configurations

Rejected. It changes resource usage, log timing, and failure continuation for
existing users without an explicit migration decision. Serial compatibility is
the safer baseline.

## References

- [ADR-0024: Add Dependency Ordering to Verification Steps](0024-verification-plan-dependencies.md)
- [ADR-0025: Establish Phase 1 Quality Baseline Gates](0025-phase-1-quality-baseline-gates.md)
- [ADR-0026: Add Configuration Safety Diagnostics and Future-Concurrency Preflight](0026-configuration-safety-diagnostics.md)
- [ADR-0027: Unify Built-in Gates and Configured Steps in a Verification Plan](0027-unified-verification-plan.md)
