# ADR 0019: Decompose Configuration Validation by Responsibility

## Status

**Accepted** - 2026-08-27

## Context

`config/validation.rs` combined configuration-wide validation with reusable
identifier, path, environment, executable, template, and step safety checks.
The mixed responsibilities made security-sensitive constraints difficult to
review in isolation.

## Decision

Split validation into private modules:

- `validation/mod`: cross-domain configuration validation and orchestration.
- `validation/primitives`: identifiers, environment names, executable names,
  OCI image references, and repository-relative paths.
- `validation/steps`: verification steps, templates, shell restrictions, and
  service injection constraints.

Keep `FlowConfig::validate`, validation order, failure-closed behavior, error
text, and all configuration contracts unchanged.

## Consequences

- Reusable security checks have a focused ownership boundary.
- Step safety rules are separated from global configuration policy.
- Existing callers retain the same validation API.

## Related

- [ADR-0008](0008-config-module-decomposition.md)
- [OpenSpec: split-config-validation](../../openspec/changes/split-config-validation/proposal.md)
