# Implementation Tasks: Split the Workflow Configuration Module

## 1. Design and Boundaries

- [x] 1.1 Record the decomposition decision in ADR-0008 (Priority: P1, Effort: S)
- [x] 1.2 Define private responsibility-based modules and preserve the existing `crate::config` boundary (Priority: P1, Effort: S)

## 2. Extraction

- [x] 2.1 Extract configuration data models and serde defaults into `config/model.rs` (Priority: P0, Effort: M)
- [x] 2.2 Extract loading, environment overrides, and lookup helpers into `config/loader.rs` (Priority: P0, Effort: M)
- [x] 2.3 Extract scope classification into `config/scope.rs` (Priority: P1, Effort: S)
- [x] 2.4 Extract validation and safety helpers into `config/validation.rs` (Priority: P0, Effort: L)
- [x] 2.5 Extract path resolution into `config/path.rs` (Priority: P1, Effort: S)
- [x] 2.6 Extract v1 migration into `config/migration.rs` (Priority: P0, Effort: M)
- [x] 2.7 Move configuration tests into `config/tests.rs` (Priority: P1, Effort: S)

## 3. Verification

- [x] 3.1 Run the complete nextest suite: 81/81 tests passed (Priority: P0, Effort: M)
- [x] 3.2 Run format, Clippy, rustdoc, and `git diff --check` (Priority: P0, Effort: S)
- [x] 3.3 Run strict OpenSpec validation; PR CI will run after submission (Priority: P0, Effort: S)
- [x] 3.4 Confirm no public CLI, configuration, migration, or error-message contract changed (Priority: P1, Effort: S)
