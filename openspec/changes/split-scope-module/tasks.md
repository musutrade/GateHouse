# Implementation Tasks: Split the Scope Module

## 1. Design and Boundaries

- [x] 1.1 Record the decomposition decision in ADR-0017 (Priority: P1, Effort: S)
- [x] 1.2 Define private error, model, detection, report, and test modules (Priority: P1, Effort: S)

## 2. Extraction

- [x] 2.1 Keep stable scope exports in `scope/mod.rs` (Priority: P0, Effort: S)
- [x] 2.2 Extract scope errors and code mapping into `scope/errors.rs` (Priority: P0, Effort: S)
- [x] 2.3 Extract scope modes and result construction into `scope/model.rs` (Priority: P0, Effort: S)
- [x] 2.4 Extract Git detection and classification into `scope/detection.rs` (Priority: P0, Effort: M)
- [x] 2.5 Extract report serialization into `scope/report.rs` (Priority: P0, Effort: S)
- [x] 2.6 Move scope tests into `scope/tests.rs` (Priority: P1, Effort: S)

## 3. Verification

- [x] 3.1 Run the complete nextest suite: 81/81 tests passed (Priority: P0, Effort: M)
- [x] 3.2 Run format, Clippy, and rustdoc (Priority: P0, Effort: S)
- [x] 3.3 Run `git diff --check` and strict OpenSpec validation (Priority: P0, Effort: S)
- [x] 3.4 Confirm no scope selection, classification, report, or error contract changed (Priority: P1, Effort: S)
