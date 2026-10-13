# Design: Split the Secret Scanner Module

## Module Layout

```text
src/secrets/mod.rs
  SecretMode, SecretsError, scan orchestration, staged/working-tree reports
src/secrets/config.rs
  SecretConfig, rule models, schema validation, compiled scanner
src/secrets/matcher.rs
  CompiledRule matching and secret-value heuristics
src/secrets/tests.rs
  Secret scanner unit tests
```

The parent keeps the existing `crate::secrets::{scan, SecretMode, SecretsError}`
boundary. Child modules use `pub(super)` only where a sibling needs an
implementation item; no child module becomes part of the crate API.

## Behavior Preservation

The extraction must preserve:

- Working-tree and staged file selection and staged configuration loading.
- Secret configuration version checks, serde fields, defaults, and validation
  messages.
- Direct, value, PostgreSQL URL, and webhook rule matching semantics.
- Placeholder exclusions, local test database exceptions, and URL handling.
- `secret_scan.json` fields, filenames, error codes, and findings ordering.

## Verification

Run the complete nextest suite, format check, Clippy with all targets and
features, rustdoc, `git diff --check`, and strict OpenSpec validation. Inspect
the diff for changed regexes, threshold checks, report fields, and visibility.

## Rollback

Revert the single refactoring commit. No secret configuration or generated
report migration is required.
