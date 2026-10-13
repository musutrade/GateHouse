# Proposal: Release Harness-Gate 0.3.2

**Status:** Implemented

## Why

PR #46 hardens scanner boundaries, cancellation, audit correctness, matcher
performance, and user-facing contracts. These changes are compatible with the
0.3.x configuration and report contracts and should be published as a patch
release.

## Goals

- Publish `harness-gate 0.3.2` to crates.io.
- Build and publish GitHub Release `v0.3.2` with the existing platform targets.
- Keep Cargo metadata, CLI version output, changelog, and documented download
  links synchronized.

## Non-goals

- No new public library API, configuration schema, or report schema.
- No changes to the release workflow or supported target matrix.

## Success Metrics

- Release validation and all required CI checks pass.
- The release workflow succeeds for tag `v0.3.2`.
- crates.io and GitHub Releases expose `0.3.2`, and docs.rs builds it.

## Risk Assessment

**Risk: Low.** This is a semver-compatible patch release from the merged
hardening PR, with package validation and cross-platform CI before tagging.

## Related Records

- [Harden Gate Boundaries and Delivery Contracts](../harden-gate-review-findings/proposal.md)
- [ADR-0031](../../../docs/adr/0031-harden-gate-boundaries.md)
