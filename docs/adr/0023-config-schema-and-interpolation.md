# ADR-0023: Generate Configuration Schema and Interpolate Environment Variables

## Status

**Accepted** (2026-08-28)

## Context

The v2 TOML model is strongly validated, but editors cannot consume its structure
without a generated schema. Configuration also has several field-specific
environment overrides, while values such as service images and timeouts need a
portable syntax that works identically in production and tests.

## Decision

Derive JSON Schema from the serde configuration model with `schemars` and expose
`harness-gate schema export`, writing `schema/flow.schema.json` by default.
Before TOML parsing, expand only `${NAME}` and `${NAME:-default}` expressions,
rejecting missing variables without defaults, malformed names, and unterminated
expressions. Existing dedicated environment overrides continue to run afterward
and therefore retain their precedence.

## Consequences

Editors and CI can validate the same model used by the binary. `load` and
`from_source` now share interpolation semantics. Interpolation deliberately does
not evaluate expressions or recurse, keeping configuration predictable and safe.
