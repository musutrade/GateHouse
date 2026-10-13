# Design: Split the Process Module

## Module Layout

```text
src/process/mod.rs
  stable crate-internal exports for process helpers and result types
src/process/signal.rs
  cancellation state and Unix signal handler installation
src/process/capture.rs
  bounded command output capture and timeout/cancellation errors
src/process/task.rs
  Task builder, log management, process execution, and TaskResult
src/process/command.rs
  process-group isolation and platform-specific termination
src/process/tests.rs
  timeout, environment, session, and capture tests
```

The parent keeps the existing `crate::process::{capture, capture_cleanup,
CapturedOutput, cancelled, install_signal_handlers, Task, TaskResult}`
boundary. Child modules expose only the helpers needed by the parent or their
neighboring implementation modules; no child module is part of the crate API.

## Behavior Preservation

The extraction must preserve:

- SIGINT/SIGTERM cancellation state and handler installation.
- Command stdout/stderr capture, reader threads, timeout errors, and cleanup
  behavior.
- Task environment additions/removals, log creation, polling intervals,
  result fields, and detail messages.
- Unix process-session isolation, process-group termination, and Windows child
  termination behavior.
- Existing public function signatures and all current test behavior.

## Verification

Run the complete nextest suite, format check, Clippy with all targets and
features, rustdoc, `git diff --check`, and strict OpenSpec validation. Inspect
the diff for changed timeout values, polling intervals, signal names,
termination behavior, environment handling, and error strings.

## Rollback

Revert the single refactoring commit. No configuration or generated artifact
migration is required.
