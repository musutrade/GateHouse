# ADR 0012: Decompose the Process Module by Responsibility

## Status

**Accepted** - 2026-08-27

## Context

`tools/harness-gate/src/process.rs` combined signal handling, command output
capture, configured task execution, process-tree isolation and termination,
and tests in one 360-line module. These concerns have different failure modes:
signal state is process-wide, capture has pipe and timeout behavior, and task
execution owns logs and result reporting. Keeping them together makes changes
to one lifecycle path harder to review.

## Decision

Split process execution into private submodules:

- `process/mod`: stable crate-internal exports for existing callers.
- `process/signal`: cancellation state and signal handler installation.
- `process/capture`: bounded command output capture and timeout errors.
- `process/task`: configured task execution, log files, and `TaskResult`.
- `process/command`: process-group isolation and termination primitives.
- `process/tests`: process and timeout behavior tests.

Keep the existing `crate::process` exports, cancellation semantics, timeout
messages, process-group behavior, environment handling, and task result shape
unchanged. Child modules remain private implementation details.

## Consequences

- Signal, capture, task, and lifecycle code can be reviewed independently.
- The parent module remains the compatibility boundary for all callers.
- Process lifecycle changes may require coordination between `command` and
  the caller module that uses it.
- Public visibility must remain limited to the existing process API.

## Alternatives Considered

- Leave the module monolithic: rejected because signal, capture, and task
  behavior have distinct ownership and risk.
- Introduce a public process abstraction: rejected because the binary crate
  only needs its current concrete helpers.
- Replace polling with an asynchronous runtime: rejected because this is a
  structural refactor and would change timing and dependency behavior.

## Related

- [ADR-0011](0011-service-module-decomposition.md)
- [OpenSpec: split-process-module](../../openspec/changes/split-process-module/proposal.md)
