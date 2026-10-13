# ADR-0024: Add Dependency Ordering to Verification Steps

## Status

**Accepted** (2026-08-28)

## Context

The refactoring plan calls for a unified verification plan before introducing parallel scheduling. Existing steps execute in configuration order and need a backwards-compatible dependency boundary first.

## Decision

Add optional `depends_on` step IDs. Configuration validation rejects missing, self, and cyclic dependencies. Selected steps are expanded to include their dependencies and emitted in a stable topological order; ties retain the order in `flow.toml`. Empty dependencies preserve legacy behavior. Execution remains serial until the later scheduler phase defines cancellation and resource rules.

## Consequences

Plans can express prerequisites without changing old presets. The dependency graph is validated before execution, and the deterministic order is available to a future scheduler. Parallelism and service locking remain out of scope for this change.
