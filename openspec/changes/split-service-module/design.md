# Design: Split the Service Module

## Module Layout

```text
src/service/mod.rs
  ServiceManager, running-service model, environment/availability entry points
src/service/postgres.rs
  External value policy and isolated PostgreSQL URL validation
src/service/docker.rs
  Docker startup, readiness, port discovery, and cleanup
src/service/tests.rs
  Service and PostgreSQL validation unit tests
```

The parent keeps the existing `crate::service::{ServiceManager, check_available}`
boundary. Child modules expose only `pub(super)` helpers required by the
parent or tests; no child module is part of the crate API.

## Behavior Preservation

The extraction must preserve:

- Service caching, failure memoization, environment injection, and lookup
  errors.
- External environment handling and `HARNESS_GATE_ALLOW_REMOTE_TEST_DATABASE`.
- PostgreSQL scheme, host, port, database suffix, and production-target checks.
- Docker command arguments, localhost port mapping, health checks, startup
  timeouts, cancellation handling, and Drop cleanup.
- Availability checks, output messages, and all existing test behavior.

## Verification

Run the complete nextest suite, format check, Clippy with all targets and
features, rustdoc, `git diff --check`, and strict OpenSpec validation. Inspect
the diff for changed Docker arguments, safety checks, timeout values, and
error strings.

## Rollback

Revert the single refactoring commit. No service configuration or generated
artifact migration is required.
