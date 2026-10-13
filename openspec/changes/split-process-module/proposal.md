# Proposal: Split the Process Module

## Why

`tools/harness-gate/src/process.rs` grew to 360 lines and combined process-wide
signal state, command output capture, task execution and logging, process-tree
lifecycle helpers, and tests. These paths have separate timeout and cleanup
risks, so responsibility-based modules make changes easier to review.

## What Changes

- Keep stable process exports in `process/mod.rs`.
- Move cancellation state and signal handlers to `process/signal.rs`.
- Move bounded output capture to `process/capture.rs`.
- Move `Task` and `TaskResult` execution to `process/task.rs`.
- Move process-group isolation and termination to `process/command.rs`.
- Move process unit tests to `process/tests.rs`.
- Preserve the existing `crate::process` exports and behavior.

## Goals

- Give signal, capture, task, and lifecycle code clear ownership.
- Keep each extracted implementation file smaller than the original module.
- Preserve cancellation, timeout, process-group, environment, logging, and
  result semantics.
- Keep extracted modules private and narrowly coupled.

## Non-goals

- Changing timeout values, polling intervals, or termination signals.
- Changing command arguments, environment inheritance, or log paths.
- Replacing process polling with asynchronous execution.
- Changing public CLI behavior or adding a library API.

## Success Metrics

- `process/mod.rs` remains the compatibility boundary for current callers.
- Existing unit, CLI, and integration tests pass without behavior changes.
- Format, Clippy with `-D warnings`, rustdoc, and strict OpenSpec validation
  pass.
- No public symbols, error messages, cancellation semantics, or process
  lifecycle behavior change.

## Risk Assessment

Low to medium. The extraction is internal to a binary crate; the main risks
are visibility mistakes and accidental changes to timeout or process-group
cleanup paths. Existing tests and static checks provide fast feedback, and
reverting the refactoring commit restores the previous layout.
