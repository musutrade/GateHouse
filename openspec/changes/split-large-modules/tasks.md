# Implementation Tasks: Split Large Production Modules

## 1. Design and Boundaries

- [x] 1.1 Record the audit module decomposition decision in ADR-0007 (Priority: P1, Effort: S)
- [x] 1.2 Define private responsibility-based modules and preserve existing public boundaries (Priority: P1, Effort: S)

## 2. Extraction

- [x] 2.1 Extract audit configuration parsing and validation into `audit/config.rs` (Priority: P0, Effort: M) - Schema types, TOML parsing, and validation now live in the private config module.
- [x] 2.2 Extract source scanning and lexical comment handling into `audit/scanner.rs` (Priority: P0, Effort: L) - Traversal, regex evaluation, allowlists, and comment-range handling now live in the private scanner module.
- [x] 2.3 Extract report models and rendering into `audit/report.rs` (Priority: P0, Effort: M) - JSON report models and Markdown rendering now live in the private report module.
- [x] 2.4 Extract structured-log parsing into `audit/log_parser.rs` (Priority: P1, Effort: S) - Trace selection and context extraction now live in the private log-parser module.
- [x] 2.5 Keep audit orchestration and stable boundary types in `audit/mod.rs` (Priority: P0, Effort: S) - `AuditError`, `AuditOutcome`, `run`, and `parse_logs` retain their existing module boundary and signatures.

## 3. Verification

- [x] 3.1 Run the complete nextest suite and verify all tests pass without behavior changes (Priority: P0, Effort: M) - `TMPDIR=/tmp cargo nextest run --manifest-path tools/harness-gate/Cargo.toml --locked`: 81/81 passed.
- [x] 3.2 Run format, Clippy, rustdoc, and strict OpenSpec validation (Priority: P0, Effort: S) - `cargo fmt`, Clippy with `-D warnings`, rustdoc, and `openspec validate split-large-modules --strict --no-interactive` passed.
- [x] 3.3 Confirm no public CLI, configuration, report, or error-code contract changed (Priority: P1, Effort: S) - Existing unit, CLI, and integration assertions pass unchanged; only private module paths and `pub(super)` interfaces were introduced.
