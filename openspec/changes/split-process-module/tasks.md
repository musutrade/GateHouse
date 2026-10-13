# Implementation Tasks: Split the Process Module

## 1. Design and Boundaries

- [x] 1.1 Record the decomposition decision in ADR-0012 (Priority: P1, Effort: S)
- [x] 1.2 Define private responsibility-based modules and preserve the existing `crate::process` boundary (Priority: P1, Effort: S)

## 2. Extraction

- [x] 2.1 Keep stable process exports in `process/mod.rs` (Priority: P0, Effort: S)
- [x] 2.2 Extract cancellation state and signal handlers into `process/signal.rs` (Priority: P0, Effort: S)
- [x] 2.3 Extract command capture into `process/capture.rs` (Priority: P0, Effort: M)
- [x] 2.4 Extract task execution and result types into `process/task.rs` (Priority: P0, Effort: M)
- [x] 2.5 Extract process lifecycle helpers into `process/command.rs` (Priority: P0, Effort: S)
- [x] 2.6 Move process tests into `process/tests.rs` (Priority: P1, Effort: S)

## 3. Verification

- [x] 3.1 Run the complete nextest suite: 81/81 tests passed (Priority: P0, Effort: M)
- [x] 3.2 Run format, Clippy, and rustdoc (Priority: P0, Effort: S)
- [x] 3.3 Run `git diff --check` and strict OpenSpec validation (Priority: P0, Effort: S)
- [x] 3.4 Confirm no public process, timeout, cancellation, or lifecycle contract changed (Priority: P1, Effort: S)
