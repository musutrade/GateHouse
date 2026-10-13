# Proposal: Configuration Safety Diagnostics and Future-Concurrency Preflight

**Status:** Implemented with renderer follow-up
**Date:** 2026-08-28
**Change type:** Configuration-contract, diagnostics, documentation, and test-plan specification

## Scope Notice

This OpenSpec change defines and implements the remaining Phase 2
configuration-system safety boundary. It adds typed validation infrastructure,
the opt-in JSON diagnostic output, safe report-template path declarations, and
their tests and documentation. It does not start services or schedule work in
parallel. Renderer behavior already present in the report boundary is tracked
by the separate confined-rendering follow-up proposal.

## Why

ADR-0023 already provides JSON Schema export and `${NAME}` /
`${NAME:-default}` interpolation. ADR-0024 already validates `depends_on` and
creates a stable serial dependency order. The current validator rejects many
invalid values, but its errors are generally prose-only and fail fast. A user
cannot reliably identify the exact field, the other end of a conflict, a
source location, or a concrete safe repair.

The current serial execution model also hides invalid relationships that would
become races when a later scheduler runs independent steps together. In
particular, services may inject the same child-process environment variable,
independent steps may acquire the same service, and steps may overwrite a
shared log. A future HTML/Tera report feature would likewise need a path policy
before it accepts template names.

Without a contract now, a scheduler or renderer would have to invent error,
resource, and filesystem behavior after external work has begun. That would
make compatibility, security review, and editor integration substantially
harder.

## What Changes

- Defines a stable, field-addressable configuration diagnostic model for TOML
  parsing, environment interpolation, semantic validation, and multi-field
  resource conflicts.
- Defines deterministic human and JSON diagnostic-rendering contracts for
  `config check`, including compatibility with the existing failure category
  and no secret disclosure.
- Defines a conservative potential-concurrency graph and static preflight
  rules for service injection names, service resources, and log files.
- Adds an optional `[report_templates]` declaration whose paths are validated
  as read-only, repository-contained template inputs; it does not add a
  renderer or template engine.
- Defines editor instructions, migration guidance, examples, negative fixtures,
  and documentation/schema validation evidence required for completion.

## Goals

1. Make every configuration error independently actionable through a stable
   diagnostic ID, canonical field path, safe reason, and deterministic repair
   guidance.
2. Let editors and CI consume diagnostics without parsing human prose while
   preserving the existing default human `config check` experience and exit
   convention.
3. Reject service-injection, shared-service, and log-output hazards before any
   project discovery, subprocess, report write, or service operation begins.
4. Define a strict report-template root and symlink-containment policy before a
   template engine can access the filesystem.
5. Synchronize editor setup, migration guidance, examples, schema generation,
   and executable validation.

## Non-goals

- No parallel scheduler, `max_parallel` setting, cancellation-policy revision,
  service lock implementation, or container-runtime abstraction.
- No HTML report, Tera dependency, template renderer, or
  change to existing JSON/Markdown report locations or names.
- No automatic insertion of dependencies, renaming of log files, or mutation
  of service environment-variable names.
- No recursive interpolation, expression evaluation, change to interpolation
  precedence, or configuration-version bump.
- No change to the existing default CLI exit status, public error headline, or
  quality snapshot contract without explicit review.

## Success Metrics

| Area | Success criterion |
| --- | --- |
| Diagnostics | Every invalid fixture yields stable IDs, canonical paths, actionable help, deterministic order, and no interpolated secret values. |
| Source context | File-backed parse, interpolation, and semantic failures report accurate one-based source positions and related conflict locations where source positions are available. |
| Machine output | `config check --format json` produces a documented versioned JSON envelope for both valid and invalid configuration; CI and editor tests parse it without prose matching. |
| Concurrency preflight | All unordered injection/service conflicts and all duplicate logs are rejected; direct and transitive ordering suppresses only the concurrency-specific findings. |
| Template safety | The future renderer acceptance suite proves lexical, canonical, and symlink containment plus report/template-root separation on supported platforms. |
| Documentation | Schema/editor association, migration paths, valid examples, and negative fixtures pass the ADR-0025 documentation-consistency check. |
| Compatibility | Existing valid presets, serial plan order, interpolation precedence, report names, error category, and reviewed CLI snapshots remain compatible. |

## Impact and Risk

**Risk: Medium.** The intended final implementation changes configuration
validation behavior, so previously loadable but ambiguous v2 files may be
rejected. It must not change execution behavior for configurations that remain
valid.

| Dimension | Impact | Control |
| --- | --- | --- |
| Security | Diagnostics or template handling could reveal secrets or allow filesystem escape. | Never render resolved values/template contents; require lexical and canonical containment plus symlink tests. |
| Compatibility | New validation can reject unsafe configurations that currently serialize and run serially. | Require explicit, deterministic diagnostics and migration guidance; preserve existing valid presets and default CLI conventions. |
| Performance | Graph analysis and source maps add load-time work. | Bound diagnostic collection to 50 entries and use linear/near-linear graph traversal; benchmark config loading only if measurements show a material regression. |
| Maintainability | Stable diagnostic IDs, JSON schema, and source-path grammar become a public contract. | Version the machine envelope, snapshot representative output, and centralize rule-to-diagnostic mappings. |
| Platform support | Windows prefixes and symlink behavior differ from Unix. | Test lexical paths on all platforms and canonical/symlink cases where the platform supports them; document unsupported fixture setup explicitly. |

## Dependencies and Assumptions

- ADR-0023's schema and interpolation syntax remain accepted v2 behavior.
- ADR-0024's `depends_on` relation is the only ordering input. The relation is
  transitive and a complete configuration has already passed missing/self/cycle
  validation before resource analysis.
- Service injection remains task-local; validators do not read service values
  from the process environment.
- `config check` remains the supported executable semantic validator; JSON
  Schema alone validates shape, not interpolation, repository containment,
  cross-references, or resource relationships.
- The quality gates introduced by ADR-0025 remain the evidence and compatibility
  boundary for the later implementation.

## Related Records

- [ADR-0026: Configuration safety diagnostics and future-concurrency preflight](../../../docs/adr/0026-configuration-safety-diagnostics.md)
- [ADR-0023: Configuration schema and interpolation](../../../docs/adr/0023-config-schema-and-interpolation.md)
- [ADR-0024: Verification plan dependencies](../../../docs/adr/0024-verification-plan-dependencies.md)
- [ADR-0025: Phase 1 quality baseline gates](../../../docs/adr/0025-phase-1-quality-baseline-gates.md)
- [Configuration reference](../../../docs/configuration.md)
