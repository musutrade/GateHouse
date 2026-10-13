# ADR 0009: Decompose the Secret Scanner by Responsibility

## Status

**Accepted** - 2026-08-27

## Context

`tools/harness-gate/src/secrets.rs` had grown to 807 lines and combined secret
configuration deserialization, regex compilation, value matching, scan-mode
orchestration, report writing, and tests. These concerns have different change
cadences and supporting dependencies, making the module difficult to review
and reason about as one unit.

## Decision

Split the secret scanner into private submodules:

- `secrets/config`: configuration models, schema validation, and compiled rule
  construction.
- `secrets/matcher`: rule matching and secret-value heuristics.
- `secrets/mod`: scan orchestration, report serialization, `SecretMode`, and
  `SecretsError`.
- `secrets/tests`: focused scanner tests.

Keep the existing `crate::secrets` exports, scan modes, report shape, error
codes, and configuration schema unchanged. Child modules remain private
implementation details.

## Consequences

- Configuration, matching, and I/O changes can be reviewed independently.
- Matching internals have an explicit boundary from filesystem and Git scan
  orchestration.
- Private visibility must be maintained when adding new rule behavior.
- Cross-cutting rule changes may touch both the compiler and matcher modules.

## Alternatives Considered

- Leave the monolith in place: rejected because the mixed ownership slows
  security-sensitive maintenance.
- Split each rule variant into a separate module: rejected because it would
  add indirection without reducing the main responsibilities.
- Introduce a public secret-scanning library: rejected because this is a
  binary crate and no external API is required.

## Related

- [ADR-0008](0008-config-module-decomposition.md)
- [OpenSpec: split-secrets-module](../../openspec/changes/split-secrets-module/proposal.md)
