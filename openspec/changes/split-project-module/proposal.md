# Proposal: Split the Project Module

## Why

`tools/harness-gate/src/project.rs` grew to 246 lines and combined project
discovery, configuration loading, repository path containment, runtime
preparation, alias expansion, and tests. Separating discovery, path security,
and runtime behavior improves reviewability without changing current callers.

## What Changes

- Keep the stable `Project` type and path helper boundary in `project/mod.rs`.
- Move root discovery, configuration loading, and validation into
  `project/discovery.rs`.
- Move repository path containment into `project/paths.rs`.
- Move runtime preparation and alias expansion into `project/runtime.rs`.
- Move project path tests into `project/tests.rs`.
- Preserve the existing `crate::project` imports and behavior.

## Goals

- Give discovery, path security, and runtime preparation clear ownership.
- Keep each extracted implementation file smaller than the original module.
- Preserve root and configuration discovery, resolved paths, validation,
  runtime filesystem effects, alias expansion, and error text.
- Keep extracted modules private and narrowly coupled.

## Non-goals

- Changing project discovery precedence or supported environment variables.
- Changing configuration loading, validation, or error messages.
- Changing path containment, symlink handling, or existence requirements.
- Changing alias names, placeholder expansion order, or preparation effects.
- Adding a filesystem abstraction, public API, or production dependency.

## Success Metrics

- `project/mod.rs` remains the compatibility boundary for current callers.
- Existing unit, CLI, and integration tests pass without behavior changes.
- Format, Clippy with `-D warnings`, rustdoc, and strict OpenSpec validation
  pass.
- No project field, method, path helper, error text, or filesystem behavior
  changes.

## Risk Assessment

Low to medium. The extraction is internal to a binary crate; the main risks
are visibility mistakes and accidental changes to path safety, discovery
precedence, or placeholder expansion order. Existing tests and static checks
provide fast feedback, and reverting the refactoring commit restores the
previous layout.
