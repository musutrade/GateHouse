# Proposal: Release Harness-Gate 0.3.1

**Status:** Implemented

## Why

The merged docs.rs fix adds a library target, but the already-published
`0.3.0` crate is immutable. A patch release is required to publish the target
and allow docs.rs to build the package successfully.

## Goals

- Publish `harness-gate 0.3.1` to crates.io.
- Build and publish GitHub Release `v0.3.1` with the existing platform targets.
- Keep Cargo metadata, CLI version output, changelog, and documented download
  links synchronized.

## Non-goals

- No runtime behavior or release-workflow changes.
- No new public library API beyond the documentation-only target.

## Success Metrics

- Release PR and all required CI checks pass.
- The release workflow succeeds for tag `v0.3.1`.
- crates.io and GitHub Releases expose `0.3.1`, and docs.rs builds it.

## Risk Assessment

**Risk: Low.** This is a semver-compatible patch release and the artifact is
independently validated before publication.

## Related Records

- [Fix docs.rs build target](../fix-docs-rs-build/proposal.md)
- [ADR-0030](../../../docs/adr/0030-docs-rs-library-target.md)
