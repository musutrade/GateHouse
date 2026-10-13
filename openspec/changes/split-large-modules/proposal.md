# Proposal: Split Large Production Modules

## Why

`tools/harness-gate/src/audit.rs` is the largest production module at more than
1,800 lines. It combines configuration, scanning, reporting, lexical parsing,
and structured-log handling. The mixed ownership increases the cost of safe
changes and makes it difficult to find the right focused tests.

## What Changes

- Move audit configuration types, parsing, and validation into a private
  `audit::config` module.
- Move source traversal, comment-range detection, and rule evaluation into a
  private `audit::scanner` module.
- Move JSON/Markdown report models and rendering into a private `audit::report`
  module.
- Move structured-log trace extraction into a private `audit::log_parser`
  module.
- Keep the existing `audit::run`, `audit::parse_logs`, error codes, reports,
  configuration schema, and CLI commands unchanged.

## Goals

- Give each audit responsibility a clear owner.
- Reduce the size of the audit orchestration module substantially.
- Preserve all existing CLI, configuration, report, and error behavior.
- Keep internal interfaces private and narrow.

## Non-goals

- Changing audit rule semantics, ignore-file behavior, or report formats.
- Changing the TOML schema or stable CLI error codes.
- Replacing the regex or Git implementations.
- Adding production dependencies or a public library API.

## Success Metrics

- `audit.rs` becomes an orchestration module rather than a monolith, with the
  extracted responsibilities in separate private files.
- Existing unit and integration tests pass without fixture or assertion changes
  caused by behavior changes.
- `cargo fmt`, Clippy with `-D warnings`, rustdoc, and OpenSpec validation pass.
- No public symbols or user-visible output change.

## Risk Assessment

Low to medium. The change moves code across private module boundaries without
altering algorithms. Rust visibility errors and the existing test suite provide
fast feedback; reverting the single refactoring commit restores the previous
layout.
