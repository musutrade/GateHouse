# Implementation Tasks: Split the Service Module

## 1. Design and Boundaries

- [x] 1.1 Record the decomposition decision in ADR-0011 (Priority: P1, Effort: S)
- [x] 1.2 Define private responsibility-based modules and preserve the existing `crate::service` boundary (Priority: P1, Effort: S)

## 2. Extraction

- [x] 2.1 Keep service caching and stable entry points in `service/mod.rs` (Priority: P0, Effort: M)
- [x] 2.2 Extract PostgreSQL policy and URL validation into `service/postgres.rs` (Priority: P0, Effort: M)
- [x] 2.3 Extract Docker lifecycle and cleanup into `service/docker.rs` (Priority: P0, Effort: M)
- [x] 2.4 Move service tests into `service/tests.rs` (Priority: P1, Effort: S)

## 3. Verification

- [x] 3.1 Run the complete nextest suite: 81/81 tests passed (Priority: P0, Effort: M)
- [x] 3.2 Run format, Clippy, and rustdoc (Priority: P0, Effort: S)
- [x] 3.3 Run `git diff --check` and strict OpenSpec validation (Priority: P0, Effort: S)
- [x] 3.4 Confirm no public service, Docker, PostgreSQL, or error-code contract changed (Priority: P1, Effort: S)
