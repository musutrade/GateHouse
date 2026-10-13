# Implementation Tasks: Split the Project Module

## 1. Design and Boundaries

- [x] 1.1 Record the decomposition decision in ADR-0016 (Priority: P1, Effort: S)
- [x] 1.2 Define private discovery, path safety, runtime, and test modules (Priority: P1, Effort: S)

## 2. Extraction

- [x] 2.1 Keep the stable `Project` type and path helper boundary in `project/mod.rs` (Priority: P0, Effort: S)
- [x] 2.2 Extract root/config discovery and validation into `project/discovery.rs` (Priority: P0, Effort: M)
- [x] 2.3 Extract repository path safety into `project/paths.rs` (Priority: P0, Effort: M)
- [x] 2.4 Extract preparation and alias expansion into `project/runtime.rs` (Priority: P0, Effort: S)
- [x] 2.5 Move project path tests into `project/tests.rs` (Priority: P1, Effort: S)

## 3. Verification

- [x] 3.1 Run the complete nextest suite: 81/81 tests passed (Priority: P0, Effort: M)
- [x] 3.2 Run format, Clippy, and rustdoc (Priority: P0, Effort: S)
- [x] 3.3 Run `git diff --check` and strict OpenSpec validation (Priority: P0, Effort: S)
- [x] 3.4 Confirm no project discovery, path safety, runtime, alias, or error contract changed (Priority: P1, Effort: S)
