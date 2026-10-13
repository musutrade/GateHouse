# Implementation Tasks: Split the Secret Scanner Module

## 1. Design and Boundaries

- [x] 1.1 Record the decomposition decision in ADR-0009 (Priority: P1, Effort: S)
- [x] 1.2 Define private responsibility-based modules and preserve the existing `crate::secrets` boundary (Priority: P1, Effort: S)

## 2. Extraction

- [x] 2.1 Extract secret configuration models and compilation into `secrets/config.rs` (Priority: P0, Effort: M)
- [x] 2.2 Extract matching and secret-value heuristics into `secrets/matcher.rs` (Priority: P0, Effort: M)
- [x] 2.3 Keep scan orchestration, reports, and stable errors in `secrets/mod.rs` (Priority: P0, Effort: S)
- [x] 2.4 Move scanner tests into `secrets/tests.rs` (Priority: P1, Effort: S)

## 3. Verification

- [x] 3.1 Run the complete nextest suite: 81/81 tests passed (Priority: P0, Effort: M)
- [x] 3.2 Run format, Clippy, and rustdoc (Priority: P0, Effort: S)
- [x] 3.3 Run `git diff --check` and strict OpenSpec validation (Priority: P0, Effort: S)
- [x] 3.4 Confirm no public scan, report, configuration, or error-code contract changed (Priority: P1, Effort: S)
