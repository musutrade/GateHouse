# Proposal: Split the Doctor Module

## Why

`tools/harness-gate/src/doctor.rs` grew to 291 lines and combined report
serialization/rendering with command, path, environment, Git, glob, version,
and service checks. Separating presentation from check execution makes changes
easier to review while keeping the diagnostic contract stable.

## What Changes

- Keep stable doctor exports in `doctor/mod.rs`.
- Move report data, counters, and rendering into `doctor/report.rs`.
- Move check dispatch and check-specific helpers into `doctor/checks.rs`.
- Move doctor tests into `doctor/tests.rs`.
- Preserve the existing `crate::doctor` exports and behavior.

## Goals

- Give report presentation and check execution clear ownership.
- Keep each extracted implementation file smaller than the original module.
- Preserve check ordering, severity counters, JSON fields, terminal output,
  timeouts, and error messages.
- Keep extracted modules private and narrowly coupled.

## Non-goals

- Changing configured check kinds, check semantics, or timeout behavior.
- Changing doctor output text, JSON schema, or severity rules.
- Adding a public doctor-check extension API.
- Changing CLI commands or introducing new dependencies.

## Success Metrics

- `doctor/mod.rs` remains the compatibility boundary for current callers.
- Existing unit, CLI, and integration tests pass without behavior changes.
- Format, Clippy with `-D warnings`, rustdoc, and strict OpenSpec validation
  pass.
- No public symbols, output fields, severity counters, or error strings change.

## Risk Assessment

Low to medium. The extraction is internal to a binary crate; the main risks
are visibility mistakes and accidental changes to serialized output or check
dispatch. Existing tests and static checks provide fast feedback, and
reverting the refactoring commit restores the previous layout.
