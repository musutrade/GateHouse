# Proposal: Split the Service Module

## Why

`tools/harness-gate/src/service.rs` grew to 470 lines and combined service
caching, environment injection, PostgreSQL isolation validation, Docker
startup/readiness/cleanup, availability checks, and tests. Security-sensitive
validation and container lifecycle changes are easier to review when separated.

## What Changes

- Keep `ServiceManager`, stable running-service entry points, and availability
  checks in `service/mod.rs`.
- Move PostgreSQL URL parsing and external-value policy validation into
  `service/postgres.rs`.
- Move Docker startup, readiness, port discovery, and cleanup into
  `service/docker.rs`.
- Move service tests into `service/tests.rs`.
- Preserve existing `crate::service` exports and behavior.

## Goals

- Give caching, database validation, and Docker lifecycle clear ownership.
- Keep implementation files smaller than the original service module.
- Preserve environment contracts, error messages, isolation checks, and
  cleanup semantics.
- Keep extracted modules private and narrowly coupled.

## Non-goals

- Changing Docker commands, health checks, port mappings, or timeouts.
- Changing PostgreSQL URL safety rules or remote-database override behavior.
- Replacing service providers or adding production dependencies.
- Changing public CLI behavior or introducing a public library API.

## Success Metrics

- The parent module contains service caching and stable entry points only.
- Existing unit, CLI, and integration tests pass without behavior changes.
- Format, Clippy with `-D warnings`, rustdoc, and strict OpenSpec validation
  pass.
- No public symbols, environment names, error strings, or lifecycle behavior
  change.

## Risk Assessment

Low to medium. The extraction stays within a binary crate; the primary risks
are private visibility mistakes and accidental changes to security checks or
cleanup paths. Existing tests and static checks provide fast feedback, and
reverting the refactoring commit restores the previous layout.
