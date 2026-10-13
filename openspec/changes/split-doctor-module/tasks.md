# Implementation Tasks: Split the Doctor Module

## 1. Design and Boundaries

- [x] 1.1 Record the decomposition decision in ADR-0013 (Priority: P1, Effort: S)
- [x] 1.2 Define private responsibility-based modules and preserve the existing `crate::doctor` boundary (Priority: P1, Effort: S)

## 2. Extraction

- [x] 2.1 Keep stable doctor exports in `doctor/mod.rs` (Priority: P0, Effort: S)
- [x] 2.2 Extract report model and rendering into `doctor/report.rs` (Priority: P0, Effort: M)
- [x] 2.3 Extract check dispatch and helpers into `doctor/checks.rs` (Priority: P0, Effort: M)
- [x] 2.4 Move doctor tests into `doctor/tests.rs` (Priority: P1, Effort: S)

## 3. Verification

- [x] 3.1 Run the complete nextest suite: 81/81 tests passed (Priority: P0, Effort: M)
- [x] 3.2 Run format, Clippy, and rustdoc (Priority: P0, Effort: S)
- [x] 3.3 Run `git diff --check` and strict OpenSpec validation (Priority: P0, Effort: S)
- [x] 3.4 Confirm no public doctor, output, severity, timeout, or error contract changed (Priority: P1, Effort: S)
