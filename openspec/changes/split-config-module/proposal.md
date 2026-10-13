# Proposal: Split the Workflow Configuration Module

## Why

`tools/harness-gate/src/config.rs` grew to 1,252 lines and combined data
models, loading, environment overrides, scope matching, validation, path
resolution, v1 migration, and tests. The mixed responsibilities make focused
changes harder to review and increase the cost of understanding dependencies.

## What Changes

- Move configuration data models and serde defaults into `config/model.rs`.
- Move TOML loading, environment overrides, and lookup helpers into
  `config/loader.rs`.
- Move changed-path classification into `config/scope.rs`.
- Move schema and cross-reference validation into `config/validation.rs`.
- Move repository-contained path resolution into `config/path.rs`.
- Move v1 migration types and logic into `config/migration.rs`.
- Move configuration unit tests into `config/tests.rs`.
- Keep the existing `crate::config` exports and behavior unchanged.

## Goals

- Give each configuration responsibility a clear owner.
- Keep each implementation file substantially smaller than the original
  monolith.
- Preserve CLI behavior, TOML compatibility, migration output, and error
  messages.
- Keep extracted modules private and narrowly coupled.

## Non-goals

- Changing the v2 configuration schema or adding new configuration features.
- Changing scope matching, validation rules, or migration semantics.
- Changing public CLI commands or introducing a public library crate.
- Replacing TOML, glob, or regex dependencies.

## Success Metrics

- `config.rs` becomes a small module wiring file with responsibility-based
  children.
- Existing unit, CLI, and integration tests pass without behavior-driven
  fixture changes.
- Format, Clippy with `-D warnings`, rustdoc, and strict OpenSpec validation
  pass.
- No public symbols, error strings, or generated output change.

## Risk Assessment

Low to medium. The extraction is mechanical and stays inside a binary crate;
the primary risks are visibility mistakes and accidental changes to serde
defaults or validation text. The complete test suite and static checks provide
fast feedback, and reverting the refactoring commit restores the prior layout.
