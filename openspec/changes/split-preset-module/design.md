# Design: Split the Preset Module

## Module Layout

```text
src/preset/mod.rs
  stable init, migrate, and print_presets exports
src/preset/catalog.rs
  embedded preset metadata and template constants
src/preset/initialize.rs
  project initialization and portable project IDs
src/preset/migration.rs
  schema v1 to v2 migration workflow
src/preset/filesystem.rs
  atomic writes, overwrite policy, and path containment
src/preset/tests.rs
  preset, initialization, path, and atomic write tests
```

The parent keeps the existing `crate::preset::{init, migrate, print_presets}`
boundary. Child modules expose only the helpers needed by sibling modules or
tests; no child module is part of the crate API.

## Behavior Preservation

The extraction must preserve:

- Embedded preset names, descriptions, TOML templates, and listing order.
- Initialization and migration paths, generated files, project IDs, output
  messages, and overwrite checks.
- Atomic temporary-file naming, syncing, replacement, and cleanup behavior.
- Path containment and symlink escape checks for all generated files.
- Existing public function signatures and all current test behavior.

## Verification

Run the complete nextest suite, format check, Clippy with all targets and
features, rustdoc, `git diff --check`, and strict OpenSpec validation. Inspect
the diff for changed include paths, generated content, output text, overwrite
rules, and path safety behavior.

## Rollback

Revert the single refactoring commit. No configuration or generated artifact
migration is required.
