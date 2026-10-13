# Design: Split Large Production Modules

## Module Layout

```text
src/audit/mod.rs
  AuditError, AuditOutcome, run/parse_logs orchestration
  tests
src/audit/config.rs
  Config and rule schema types
  TOML parsing and validation
src/audit/scanner.rs
  Regex compilation and path resolution
  Lexical comment-range detection
  Hard-rule and architecture-rule scans
src/audit/report.rs
  JSON report models and serialization
  Markdown rendering
src/audit/log_parser.rs
  Structured-log trace selection and context extraction
```

The parent module imports only the child functions and data types it needs.
Child modules expose implementation items with `pub(super)` where the parent
or a sibling needs access; no child module becomes part of the crate API.

## Behavior Preservation

The extraction is mechanical at the algorithm level. Keep the following
contracts unchanged:

- `audit::run` returns the same `AuditOutcome` and `E1101`/`E1102` mappings.
- `audit::parse_logs` returns the same `E1103` mapping and output shape.
- Audit TOML validation and migration messages remain unchanged.
- Ignore-file traversal, lexical comment handling, and allowlist matching stay
  in the scanner/config ownership chosen by ADR-0007.
- JSON and Markdown filenames, truncation, and emitted fields remain unchanged.

## Verification

Run the existing full test suite and static checks after extraction. Inspect the
diff for accidental changes to string literals, serde attributes, report field
names, and visibility. Validate the OpenSpec change with strict mode.

## Rollback

Revert the single refactoring commit. No configuration, data, or generated
artifacts require migration.
