# Proposal: Split the Verification Module

## Why

`tools/harness-gate/src/verify.rs` grew to 485 lines and combined stable
verification errors and reports with gate orchestration, service-backed task
execution, parser logic, output helpers, and tests. This makes focused changes
harder to review and reason about.

## What Changes

- Keep errors, reports, public entry points, and gate orchestration in
  `verify/mod.rs`.
- Move configured service setup, task construction, execution, and result
  output into `verify/steps.rs`.
- Move ANSI normalization and configurable result parsing into
  `verify/parser.rs`.
- Move verification unit tests into `verify/tests.rs`.
- Preserve all existing `crate::verify` exports and behavior.

## Goals

- Give orchestration, task execution, and parsing clear ownership boundaries.
- Keep implementation files smaller than the original module.
- Preserve step selection, report contents, console output, and error codes.
- Keep extracted modules private and narrowly coupled.

## Non-goals

- Changing verification profiles, scope selection, or service semantics.
- Changing parser patterns, ANSI handling, or report filenames and fields.
- Replacing the process, service, or regex implementations.
- Adding production dependencies or a public library API.

## Success Metrics

- The parent module contains stable workflow boundaries and orchestration only.
- Existing unit, CLI, and integration tests pass without behavior changes.
- Format, Clippy with `-D warnings`, rustdoc, and strict OpenSpec validation
  pass.
- No public symbols, error strings, output text, or report shapes change.

## Risk Assessment

Low to medium. The extraction stays within a binary crate; the primary risks
are private visibility mistakes and accidental changes to task ordering or
parser thresholds. Existing tests and static checks provide fast feedback,
and reverting the refactoring commit restores the previous layout.
