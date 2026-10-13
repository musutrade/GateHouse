# Proposal: Split the Preset Module

## Why

`tools/harness-gate/src/preset.rs` grew to 281 lines and combined embedded
preset definitions, initialization, schema migration, atomic writes, path
containment checks, and tests. Separating catalog, workflows, and filesystem
policy makes changes easier to review without changing the CLI contract.

## What Changes

- Keep stable preset exports in `preset/mod.rs`.
- Move embedded preset metadata and templates into `preset/catalog.rs`.
- Move project initialization into `preset/initialize.rs`.
- Move schema migration into `preset/migration.rs`.
- Move atomic write and path safety helpers into `preset/filesystem.rs`.
- Move preset tests into `preset/tests.rs`.
- Preserve the existing `crate::preset` exports and behavior.

## Goals

- Give catalog, initialization, migration, and filesystem safety clear
  ownership.
- Keep each extracted implementation file smaller than the original module.
- Preserve preset output, generated content, overwrite policy, migration
  semantics, and path containment behavior.
- Keep extracted modules private and narrowly coupled.

## Non-goals

- Changing preset names, descriptions, templates, or generated paths.
- Changing initialization or migration output text and overwrite behavior.
- Changing atomic write semantics or repository path safety rules.
- Adding a preset plugin API or new production dependencies.

## Success Metrics

- `preset/mod.rs` remains the compatibility boundary for current callers.
- Existing unit, CLI, and integration tests pass without behavior changes.
- Format, Clippy with `-D warnings`, rustdoc, and strict OpenSpec validation
  pass.
- No public symbols, output text, generated files, or security checks change.

## Risk Assessment

Low to medium. The extraction is internal to a binary crate; the main risks
are include path mistakes, visibility errors, and accidental changes to
filesystem safety or generated content. Existing tests and static checks
provide fast feedback, and reverting the refactoring commit restores the
previous layout.
