# Design: Complete Audit Boundary Decomposition

## Module Layout

```text
src/audit/mod.rs
  stable public audit boundary
src/audit/errors.rs
  typed errors and error-code mapping
src/audit/model.rs
  public outcome and internal violations
src/audit/runner.rs
  config loading, scan orchestration, report output, log entry point
src/audit/tests.rs
  audit boundary coverage
```

Existing `config`, `scanner`, `report`, and `log_parser` modules retain their
current ownership. The parent preserves `crate::audit::{run, parse_logs,
AuditError, AuditOutcome}`; no child module becomes part of the crate API.

## Behavior Preservation

Preserve configuration read/parse order, hard-rule loop order, architecture
scan timing, report serialization, UTF-8-safe Markdown truncation, JSON
stdout emission, log-parser error context, and all typed error messages/codes.

## Verification

Run complete nextest, format, Clippy with all targets/features, rustdoc,
`git diff --check`, and strict OpenSpec validation. Inspect the diff for public
exports, runner ordering, report writes, truncation, stdout output, and error
mapping.

## Rollback

Revert the single refactoring commit. No configuration or artifact migration is
required.
