# ADR-0030: Provide a Documentation Target for the CLI Crate

## Status

**Accepted** (2026-08-29)

## Context

The `harness-gate` package is intentionally distributed as a binary. Its
implementation modules are private, and earlier decomposition decisions
explicitly rejected exposing them as a public library API. docs.rs nevertheless
invokes `cargo rustdoc --lib` for every package. Version `0.3.0` therefore
failed before rustdoc could run with `no library targets found in package
harness-gate`.

## Decision

Add a minimal `src/lib.rs` containing crate-level documentation only. The
library target must not re-export the CLI's internal modules or introduce a
runtime API. The existing binary target remains the supported interface and
keeps its current module boundaries and behavior.

Future public library functionality requires a separate ADR and compatibility
review. Because crates.io artifacts are immutable, this fix takes effect for a
new patch release rather than changing the already-published `0.3.0` package.

## Consequences

- Future releases have a valid library target for docs.rs and no longer fail
  during the pre-build target check.
- docs.rs presents crate-level usage documentation rather than an API surface;
  this accurately reflects the CLI-only support contract.
- The package contains a small additional target, with negligible build cost.
- The published `0.3.0` docs remain immutable and require a follow-up patch
  release to expose the fixed documentation.

## Alternatives Considered

- Configure docs.rs targets or features only: rejected because metadata cannot
  create the missing library target.
- Re-export the binary's internal modules: rejected because it would create an
  accidental public API and contradict the existing architecture decisions.
- Remove the docs.rs badge: rejected because it hides a broken documentation
  contract instead of fixing it.

## Related

- [ADR-0007: Decompose the audit module by responsibility](0007-audit-module-decomposition.md)
- [ADR-0008: Decompose the workflow configuration module by responsibility](0008-config-module-decomposition.md)
- [OpenSpec: Fix the docs.rs build target](../../openspec/changes/fix-docs-rs-build/proposal.md)
