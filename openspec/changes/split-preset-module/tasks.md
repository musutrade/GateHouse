# Implementation Tasks: Split the Preset Module

## 1. Design and Boundaries

- [x] 1.1 Record the decomposition decision in ADR-0014 (Priority: P1, Effort: S)
- [x] 1.2 Define private responsibility-based modules and preserve the existing `crate::preset` boundary (Priority: P1, Effort: S)

## 2. Extraction

- [x] 2.1 Keep stable preset exports in `preset/mod.rs` (Priority: P0, Effort: S)
- [x] 2.2 Extract embedded preset catalog and templates into `preset/catalog.rs` (Priority: P0, Effort: S)
- [x] 2.3 Extract initialization into `preset/initialize.rs` (Priority: P0, Effort: M)
- [x] 2.4 Extract schema migration into `preset/migration.rs` (Priority: P0, Effort: M)
- [x] 2.5 Extract filesystem safety helpers into `preset/filesystem.rs` (Priority: P0, Effort: M)
- [x] 2.6 Move preset tests into `preset/tests.rs` (Priority: P1, Effort: S)

## 3. Verification

- [x] 3.1 Run the complete nextest suite: 81/81 tests passed (Priority: P0, Effort: M)
- [x] 3.2 Run format, Clippy, and rustdoc (Priority: P0, Effort: S)
- [x] 3.3 Run `git diff --check` and strict OpenSpec validation (Priority: P0, Effort: S)
- [x] 3.4 Confirm no public preset, output, generated-file, overwrite, or path-safety contract changed (Priority: P1, Effort: S)
