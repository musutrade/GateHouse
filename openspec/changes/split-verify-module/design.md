# Design: Split the Verification Module

## Module Layout

```text
src/verify/mod.rs
  VerifyError, VerificationReport, public entry points, gate orchestration
src/verify/steps.rs
  ServiceManager setup, Task construction/execution, result output
src/verify/parser.rs
  ANSI normalization and configurable result-count parsing
src/verify/tests.rs
  Verification unit tests
```

The parent keeps the existing `crate::verify` boundary. Child modules expose
only `pub(super)` helpers required by the parent or tests; no child module is
part of the crate API.

## Behavior Preservation

The extraction must preserve:

- Profile and explicit-step selection, scope handling, and gate ordering.
- Secret and architecture gate results and their report entries.
- Service setup failures, task environment isolation, cancellation, and
  timeout behavior.
- Parser regex patterns, ANSI stripping, capture handling, and thresholds.
- JSON/Markdown report fields, console output, filenames, and error codes.

## Verification

Run the complete nextest suite, format check, Clippy with all targets and
features, rustdoc, `git diff --check`, and strict OpenSpec validation. Inspect
the diff for changed task order, strings, parser thresholds, and visibility.

## Rollback

Revert the single refactoring commit. No generated report or configuration
migration is required.
