# Proposal: Complete Audit Boundary Decomposition

## Why

The prior audit decomposition separated configuration, scanning, reporting,
and log parsing, but `audit/mod.rs` still held its boundary types, orchestration,
and tests. Splitting the remaining responsibilities makes production behavior
and test coverage easier to review independently.

## What Changes

- Keep stable audit exports in `audit/mod.rs`.
- Move typed errors and code mapping into `audit/errors.rs`.
- Move audit outcome and violation models into `audit/model.rs`.
- Move configuration loading, scan orchestration, and report output into
  `audit/runner.rs`.
- Move audit tests into `audit/tests.rs`.
- Preserve the existing `crate::audit` API and behavior.

## Goals

- Keep the public audit boundary compact and explicit.
- Preserve config loading, scanning, report output, JSON emission, log parsing,
  error messages, and error codes exactly.
- Keep extracted modules private and narrowly coupled.

## Non-goals

- Changing audit configuration schema, rule behavior, scanning, or reports.
- Changing log parsing, public function signatures, error formatting, or
  production dependencies.
- Adding an audit extension API or changing test coverage scope.

## Success Metrics

- `audit/mod.rs` only wires private modules and re-exports the stable API.
- Existing unit, CLI, and integration tests pass without behavior changes.
- Format, Clippy with `-D warnings`, rustdoc, and strict OpenSpec validation
  pass.
- No audit API, report, error, or output behavior changes.

## Risk Assessment

Low. The extraction is internal to the binary crate. The primary risks are
visibility mistakes and accidental changes to runner ordering or report
truncation; the full test suite and static checks provide coverage.
