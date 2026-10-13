# Proposal: Split the Scope Module

## Why

`tools/harness-gate/src/scope.rs` grew to 227 lines and combined typed errors,
scope models, Git path selection, configuration classification, report writing,
and tests. Separating those concerns improves reviewability without changing
the existing scope API or CLI behavior.

## What Changes

- Keep stable scope exports in `scope/mod.rs`.
- Move error types and code mapping into `scope/errors.rs`.
- Move scope modes and result construction into `scope/model.rs`.
- Move Git detection and path classification into `scope/detection.rs`.
- Move report serialization into `scope/report.rs`.
- Move scope tests into `scope/tests.rs`.
- Preserve the existing `crate::scope` boundary and behavior.

## Goals

- Give errors, models, detection, classification, and reporting clear
  ownership.
- Keep each extracted implementation file smaller than the original module.
- Preserve Git command ordering, changed-path de-duplication, base-reference
  validation, unmatched-file policy, report contents, and error formatting.
- Keep extracted modules private and narrowly coupled.

## Non-goals

- Changing scope modes, CLI options, or detection defaults.
- Changing Git commands, arguments, or path decoding behavior.
- Changing configuration classification or unmatched-file handling.
- Changing report paths, file contents, JSON shape, or output ordering.
- Adding a scope-provider abstraction, public API, or production dependency.

## Success Metrics

- `scope/mod.rs` remains the compatibility boundary for current callers.
- Existing unit, CLI, and integration tests pass without behavior changes.
- Format, Clippy with `-D warnings`, rustdoc, and strict OpenSpec validation
  pass.
- No scope type, detection result, report, error text, or error code changes.

## Risk Assessment

Low to medium. The extraction is internal to a binary crate; the main risks
are visibility mistakes and accidental changes to Git command ordering,
classification policy, report serialization, or error mapping. Existing tests
and static checks provide fast feedback, and reverting the refactoring commit
restores the previous layout.
