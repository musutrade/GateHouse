# ADR 0008: Decompose the Workflow Configuration Module by Responsibility

## Status

**Accepted** - 2026-08-27

## Context

`tools/harness-gate/src/config.rs` had grown into a 1,252-line module that
combined configuration data models, TOML loading, environment overrides, scope
matching, validation, path resolution, v1 migration, and tests. These concerns
change at different rates and require different supporting dependencies, which
made focused maintenance harder than necessary.

## Decision

Split the workflow configuration implementation into private submodules:

- `config/model`: public configuration data models and serde defaults.
- `config/loader`: TOML loading, environment overrides, and lookup helpers.
- `config/scope`: changed-path classification against configured scope rules.
- `config/validation`: schema and cross-reference validation helpers.
- `config/path`: repository-contained configuration path resolution.
- `config/migration`: v1 schema types and migration to v2.
- `config/tests`: focused unit tests for the configuration boundary.

Keep the existing `crate::config` exports, method signatures, TOML schema,
error messages, and migration behavior unchanged. The child modules remain
private implementation details.

## Consequences

- Configuration concerns have smaller files and explicit ownership.
- Validation and migration can evolve independently from runtime loading.
- Internal module boundaries prevent accidental coupling to implementation
  details while preserving the current crate-level API.
- Cross-cutting configuration changes may require coordinating more than one
  private module.

## Alternatives Considered

- Leave the monolith in place: rejected because its size and mixed concerns
  slow safe maintenance.
- Split every function into its own module: rejected because it would create
  excessive indirection without useful ownership boundaries.
- Expose a new configuration library API: rejected because Harness Gate is a
  binary crate and no external API is required.

## Related

- [ADR-0007](0007-audit-module-decomposition.md)
- [OpenSpec: split-config-module](../../openspec/changes/split-config-module/proposal.md)
