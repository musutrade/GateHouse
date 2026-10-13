# Parallel Scheduling Specification

## Implementation Plan and Timeline

This specification is the normative contract for Phase 4. The timeline is a
delivery order, not permission to change production behavior before the
corresponding evidence is accepted.

| Window | Deliverable | Evidence gate |
| --- | --- | --- |
| T+0 to T+1 day | Capture serial behavior and approve execution-policy schema | Legacy snapshots and invalid-policy diagnostics are reviewed |
| T+2 to T+3 days | Specify scheduler state, readiness, dispatch limits, and failure propagation | Controlled graph fixtures cover serial, bounded parallel, descendants, and unrelated branches |
| T+4 to T+5 days | Specify service locks, task-local contexts, log allocation, and ordered publication | Resource, collision, cleanup, and reordered-completion fixtures pass |
| T+6 to T+7 days | Prepare cancellation, contention, benchmark, and compatibility evidence | ADR-0025-compatible artifacts and cross-platform contract results are reviewed |

## Technical Examples

The only new configuration surface is:

```toml
[execution]
parallel = true
max_parallel = 4
```

The logical internal policy is:

```text
ExecutionPolicy {
  parallel: bool,          # default false
  max_parallel: 1..=64,    # default 4 when parallel is true
  failure_policy: explicit,
  timeout_policy: existing boundary
}
```

These examples define data contracts only. They do not prescribe Rust module
names, threading libraries, or executable implementation.

## ADDED Requirements

### Requirement: Explicit execution policy

The configuration SHALL accept an optional `[execution]` section with boolean
`parallel` and integer `max_parallel`. Omitted configuration SHALL preserve
serial behavior. When `parallel = true` and `max_parallel` is omitted, the
effective bound SHALL be 4. Explicit bounds SHALL be 1 through 64 inclusive.
Invalid values SHALL fail before plan execution or any external side effect.

#### Scenario: Legacy configuration remains serial

- **WHEN** a valid configuration has no `[execution]` section
- **THEN** verification executes serially using the existing plan order
- **AND THEN** CLI output, reports, logs, and error codes remain compatible

#### Scenario: Invalid parallel bound fails closed

- **WHEN** `max_parallel` is zero, negative, or greater than 64
- **THEN** configuration validation fails before a service, process, or report
  operation begins
- **AND THEN** the configuration is not silently downgraded to serial

### Requirement: Dependency-aware bounded dispatch

The scheduler SHALL consume the validated `VerificationPlan` and its stable
topological index. A node SHALL be ready only after every dependency has
passed. Parallel dispatch SHALL never exceed the effective `max_parallel`; serial
mode SHALL use the same queue with one active node. Ready ties SHALL follow
stable topological order and the existing configuration-order tie breaker.

#### Scenario: Independent nodes run within the limit

- **WHEN** two selected nodes have no dependency path between them and
  `parallel = true, max_parallel = 2`
- **THEN** both may run concurrently
- **AND THEN** no third node starts until a worker slot is available

#### Scenario: Dependency blocks early dispatch

- **WHEN** node `b` depends on node `a`
- **THEN** `b` does not start while `a` is pending or running
- **AND THEN** `b` starts only after `a` has a passed result

#### Scenario: Mandatory legacy gates remain ordered

- **WHEN** a legacy configuration enables parallel mode
- **THEN** synthesized `secret-scan -> architecture-audit -> external` edges
  remain enforced
- **AND THEN** only eligible independent external branches can overlap

### Requirement: Shared service safety

The scheduler SHALL use adapter-declared service resource identities and runtime
locks in addition to ADR-0026 static preflight. Startup and teardown SHALL be
exclusive. Shareable ready services MAY serve multiple node contexts; a
non-shareable service SHALL be exclusive to one node. Lock waits SHALL observe
cancellation and a bounded wait. Failed startup SHALL release ownership, and
cleanup SHALL occur exactly once after the final user for every terminal state.

#### Scenario: Reusable service starts once

- **WHEN** two eligible nodes use the same adapter-declared shareable service
- **THEN** only one startup transition occurs
- **AND THEN** both nodes receive the same ready managed instance
- **AND THEN** teardown occurs once after both nodes release it

#### Scenario: Exclusive service prevents overlap

- **WHEN** two eligible nodes request a non-shareable resource
- **THEN** only one holds the resource at a time
- **AND THEN** the other waits or becomes terminal according to the explicit
  lock policy, without duplicate startup or concurrent use

#### Scenario: Cancellation while waiting releases the lock request

- **WHEN** a node is cancelled while waiting for a service lock
- **THEN** it becomes cancelled without acquiring the resource
- **AND THEN** existing owners still receive normal cleanup

### Requirement: Task-local execution and unique logs

Each node SHALL receive a task-local execution context. Service injection SHALL
affect only the child process context and SHALL NOT mutate the parent process
environment. Before dispatch, every selected external node SHALL receive a
unique normalized log path contained by the report-output root. Collisions or
unsafe paths SHALL fail before execution.

#### Scenario: Parallel logs do not overwrite

- **WHEN** two independent external nodes execute concurrently
- **THEN** each writes only to its own unique log path
- **AND THEN** both logs remain complete after the run

#### Scenario: Normalization collision fails before side effects

- **WHEN** two configured log paths normalize to the same identity
- **THEN** validation rejects the plan
- **AND THEN** no service or process starts

### Requirement: Stable result publication

Workers SHALL return `NodeResult` values without publishing completion-order
summaries. A single publisher SHALL merge and print results in stable plan
order, preserving existing labels, report paths, report shape, ANSI policy, and
summary semantics. Completion timestamps and worker interleavings SHALL NOT
reorder public output. Report-write failures SHALL use `E1404`.

#### Scenario: Completion order does not alter output

- **WHEN** the same fixture completes independent nodes in two different
  worker orders
- **THEN** normalized stdout, stderr, and reports are equivalent and ordered by
  the stable plan order
- **AND THEN** per-node duration may differ without changing ordering

### Requirement: Unified cancellation and timeout

Cancellation SHALL be a plan-level event observed before dispatch, while
waiting for a lock, and by running adapters. No unstarted node SHALL begin
after cancellation is observed. Running external work SHALL use the existing
process-tree termination path and bounded cleanup. Timeout SHALL be represented
as a typed cancellation reason, retain the node log, block descendants, and
perform cleanup. Cancelled or skipped nodes SHALL never satisfy a dependency or
be reported as passed.

#### Scenario: Running node is cancelled

- **WHEN** cancellation arrives while an external node is running
- **THEN** the process tree is terminated through the existing boundary
- **AND THEN** the node is recorded as cancelled and descendants do not start
- **AND THEN** the command preserves the established `E1402` behavior

#### Scenario: Timeout blocks descendants

- **WHEN** a node exceeds its effective timeout
- **THEN** the node is recorded with a timeout cancellation reason
- **AND THEN** its descendants are blocked and its log/cleanup evidence remains
- **AND THEN** no node is reported as passed because of the timeout

### Requirement: Deterministic failure propagation

A failed node SHALL block all descendants. In parallel mode, unrelated ready
branches MAY continue until the explicit plan-level fail-fast policy requests
cancellation. Serial mode SHALL preserve the captured pre-scheduler behavior.
The final result SHALL be failed if any selected node fails, times out, or is
cancelled. If multiple nodes fail, stable plan order SHALL choose the primary
public error; execution failures SHALL preserve `E1403`.

#### Scenario: Failed branch blocks descendants only

- **WHEN** node `a` fails and node `b` depends on `a`
- **THEN** `b` is skipped or blocked according to the documented status policy
- **AND THEN** an unrelated selected node follows the configured continuation
  policy

#### Scenario: Multiple failures select a stable primary error

- **WHEN** two independent nodes fail in different completion orders
- **THEN** the primary public failure is selected by stable plan order
- **AND THEN** both node failures remain available as structured details

### Requirement: Compatibility and performance evidence

The implementation SHALL provide regression snapshots for serial compatibility
and structured tests for parallel execution on supported platforms. It SHALL
provide fixed-fixture benchmarks for serial and parallel wall time, per-step
duration, peak concurrency, scheduler overhead, and service startup reuse. It
SHALL record ADR-0025 comparison metadata and SHALL include repeated contention
runs that assert no process, service, lock, or log leaks.

#### Scenario: Serial snapshot remains compatible

- **WHEN** a legacy fixture is run with no execution section
- **THEN** output, report paths/names/shape, logs, exit status, and public error
  codes match the pre-scheduler reference

#### Scenario: Repeated parallel runs are stable

- **WHEN** a fixed parallel fixture is run repeatedly with controlled completion
  timing
- **THEN** normalized public output and reports remain equivalent
- **AND THEN** peak concurrency stays within the configured bound
- **AND THEN** cleanup leaves no managed artifact behind

## Alternatives Considered

See [design.md](../../design.md#alternatives-considered) and ADR-0028 for the
rejected call-site workers, completion-order publication, lock-only safety,
service duplication, and parallel-by-default alternatives.

## Rollback Plan

If implementation evidence fails, revert the scheduler integration and
execution-policy handling as one reviewed change. Since `[execution]` is
optional, existing configurations remain readable and execution returns to the
serial path. Rollback must preserve existing reports and clean all managed
processes/services; it must not rewrite user configuration.
