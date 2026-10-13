# Design: Split the Workflow Configuration Module

## Module Layout

```text
src/config/mod.rs
  crate::config exports and private module wiring
src/config/model.rs
  FlowConfig and all serde data models/defaults
src/config/loader.rs
  TOML loading, environment overrides, and lookup helpers
src/config/scope.rs
  Changed-path classification against scope rules
src/config/validation.rs
  Schema, cross-reference, template, and safety validation
src/config/path.rs
  Repository-contained configuration path resolution
src/config/migration.rs
  v1 compatibility types and migration to v2
src/config/tests.rs
  Configuration unit tests
```

The parent module re-exports the same configuration symbols that were defined
in the original `config.rs`. Child modules use private imports and implement
additional `FlowConfig` methods in separate `impl` blocks; no child module is
part of the crate's public module tree.

## Behavior Preservation

The extraction must preserve:

- `FlowConfig::load` and `FlowConfig::from_source` parsing and environment
  override precedence.
- Scope classification, validation rules, and all existing error messages.
- `resolve_config_path` repository containment checks.
- v1 migration output, defaults, and the v2 schema version.
- Existing serde field names, defaults, and deny-unknown-fields behavior.

## Verification

Run the complete nextest suite, format check, Clippy with all targets and
features, rustdoc, `git diff --check`, and strict OpenSpec validation. Inspect
the diff for accidental changes to public re-exports, serde attributes,
validation strings, and migration defaults.

## Rollback

Revert the single refactoring commit. No configuration files or generated
artifacts require migration.
