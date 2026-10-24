# ADR 0011: Decompose the Service Module by Responsibility

Current-source note: the former `service/docker`, `service/commands`,
`service/runtime`, `service/inspection` and `service/legacy_cli` modules have been
retired. Engine operations now go through installed lifecycle hosts; Core retains
lease ownership, diagnostics and recovery coordination. The decomposition below
records the earlier architecture. See [migration guidance](../runtime-lifecycle-v1.md#legacy-cli-retirement).

## Status

**Accepted** - 2026-08-27

## Context

`tools/harness-gate/src/service.rs` combined service caching and failure
handling, external PostgreSQL URL validation, Docker startup and health checks,
availability checks, cleanup, and tests in one 470-line module. Security
validation and container lifecycle code have different change patterns and
should be independently reviewable.

## Decision

Split service implementation into private submodules:

- `service/mod`: `ServiceManager`, running-service model, stable service
  environment and availability entry points.
- `service/postgres`: external-value policy and isolated PostgreSQL URL parsing
  and validation.
- `service/docker`: Docker start, readiness, port discovery, and cleanup.
- `service/tests`: focused service and PostgreSQL validation tests.

Keep the existing `crate::service` exports, environment-variable contracts,
Docker behavior, PostgreSQL isolation checks, error messages, and cleanup
semantics unchanged. Child modules remain private implementation details.

## Consequences

- Database safety checks are separated from Docker process lifecycle code.
- Service caching and public entry points remain easy to locate.
- Private visibility must be maintained when extending service providers.
- Cross-cutting service changes may touch the parent and one implementation
  module.

## Alternatives Considered

- Leave the module monolithic: rejected because security and lifecycle logic
  have distinct ownership.
- Split each service provider into a public abstraction: rejected because the
  current binary crate only needs private Docker and environment providers.
- Replace Docker with a new runtime: rejected because this change is a
  structural refactor, not a provider change.

## Related

- [ADR-0010](0010-verify-module-decomposition.md)
- [OpenSpec: split-service-module](../../openspec/changes/split-service-module/proposal.md)
