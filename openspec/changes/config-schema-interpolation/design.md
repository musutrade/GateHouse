# Design: Configuration Schema and Environment Interpolation

`schemars::JsonSchema` is derived alongside serde on every v2 configuration
type. The CLI serializes `schema_for!(FlowConfig)` to the requested output path.

Interpolation is a small scanner over the source text before TOML parsing. It
accepts ASCII environment names and an optional literal default after `:-`.
After parsing, existing field-specific overrides are applied and validation is
unchanged. This preserves old configs while making `load` and `from_source`
consistent.

Rollback is a revert of the dependency, loader, CLI, and generated schema; no
runtime data migration is involved.
