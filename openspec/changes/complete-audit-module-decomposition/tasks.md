# Implementation Tasks: Complete Audit Boundary Decomposition

## 1. Design and Boundaries

- [x] 1.1 Record the boundary decomposition in ADR-0018 (Priority: P1, Effort: S)
- [x] 1.2 Define private error, model, runner, and test modules (Priority: P1, Effort: S)

## 2. Extraction

- [x] 2.1 Keep stable audit exports in `audit/mod.rs` (Priority: P0, Effort: S)
- [x] 2.2 Extract typed errors and code mapping into `audit/errors.rs` (Priority: P0, Effort: S)
- [x] 2.3 Extract outcomes and violation models into `audit/model.rs` (Priority: P0, Effort: S)
- [x] 2.4 Extract audit run orchestration into `audit/runner.rs` (Priority: P0, Effort: M)
- [x] 2.5 Move audit tests into `audit/tests.rs` (Priority: P1, Effort: S)

## 3. Verification

- [x] 3.1 Run the complete nextest suite: 81/81 tests passed (Priority: P0, Effort: M)
- [x] 3.2 Run format, Clippy, and rustdoc (Priority: P0, Effort: S)
- [x] 3.3 Run `git diff --check` and strict OpenSpec validation (Priority: P0, Effort: S)
- [x] 3.4 Confirm no audit API, scan, report, log, or error contract changed (Priority: P1, Effort: S)
