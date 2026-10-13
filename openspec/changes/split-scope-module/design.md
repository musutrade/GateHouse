# Design: Split the Scope Module

## Module Layout

```text
src/scope/mod.rs
  stable scope exports
src/scope/errors.rs
  typed scope errors and error-code mapping
src/scope/model.rs
  scope modes and result construction
src/scope/detection.rs
  Git worktree checks, changed paths, and classification
src/scope/report.rs
  changed-file and JSON report writing
src/scope/tests.rs
  scope classification tests
```

The parent keeps the existing `crate::scope::{ScopeError, ScopeMode,
ScopeResult, detect}` boundary. Child modules expose only the implementation
needed by the parent or sibling modules; no child module becomes part of the
crate API.

## Behavior Preservation

The extraction must preserve:

- Scope mode variants, result fields, and all-component behavior.
- Git worktree validation and exact command ordering for working-tree, staged,
  and base modes.
- NUL-delimited path decoding, de-duplication, base-reference validation, and
  range selection.
- Configuration classification, unmatched-file policy, component ordering,
  and error codes/messages.
- Changed-file report contents, JSON shape, report paths, and write ordering.
- Existing public function signatures and all current test behavior.

## Verification

Run the complete nextest suite, format check, Clippy with all targets and
features, rustdoc, `git diff --check`, and strict OpenSpec validation. Inspect
the diff for changed Git arguments, branch ordering, unmatched-file handling,
report contents, and error formatting.

## Rollback

Revert the single refactoring commit. No configuration or generated artifact
migration is required.
