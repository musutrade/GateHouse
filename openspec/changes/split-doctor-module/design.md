# Design: Split the Doctor Module

## Module Layout

```text
src/doctor/mod.rs
  stable run and DoctorReport exports
src/doctor/report.rs
  serialized report model, severity counters, and terminal rendering
src/doctor/checks.rs
  configured check dispatch and command/path/Git/service helpers
src/doctor/tests.rs
  Git remote and doctor behavior tests
```

The parent keeps the existing `crate::doctor::{run, DoctorReport}` boundary.
Child modules expose only the helpers needed by the parent or test module; no
child module is part of the crate API.

## Behavior Preservation

The extraction must preserve:

- Doctor report JSON field names, check ordering, counters, and terminal text.
- Required versus optional check severity and help text composition.
- Command output handling, path and glob expansion, environment checks, Git
  remote credential detection, version checks, and service availability.
- Per-check timeout values, deadline handling, error messages, and existing
  CLI behavior.
- Existing public function signatures and all current test behavior.

## Verification

Run the complete nextest suite, format check, Clippy with all targets and
features, rustdoc, `git diff --check`, and strict OpenSpec validation. Inspect
the diff for changed output fields, severity logic, check ordering, timeout
handling, and error strings.

## Rollback

Revert the single refactoring commit. No configuration or generated artifact
migration is required.
