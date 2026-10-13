# Proposal: Fix the docs.rs Build Target

**Status:** Implemented
**Date:** 2026-08-29
**Change type:** Documentation packaging and release metadata

## Why

docs.rs builds the library target with `cargo rustdoc --lib`. The published
`harness-gate 0.3.0` package contains only a binary target, so docs.rs stops
before rustdoc with `no library targets found in package harness-gate`.

## Goals

- Give the package a valid rustdoc library target for future releases.
- Keep the binary as the supported interface and preserve all existing module
  visibility and runtime behavior.
- Document the packaging decision and verify the same target locally.

## Non-goals

- No public re-export of CLI implementation modules.
- No new library runtime API or feature flags.
- No attempt to modify the immutable `0.3.0` crates.io artifact.

## Success Metrics

| Area | Success criterion |
| --- | --- |
| Rustdoc | `cargo doc --manifest-path tools/harness-gate/Cargo.toml --lib --no-deps` succeeds. |
| Packaging | `cargo package --manifest-path tools/harness-gate/Cargo.toml --allow-dirty` includes `src/lib.rs`. |
| Compatibility | Existing tests, CLI behavior, formatting, and Clippy checks remain green. |
| Documentation | ADR-0030 and this change explain why the target is not a public runtime API. |
| Release | A patch release after merge produces a successful docs.rs build. |

## Risk Assessment

**Risk: Low.** The new target contains documentation only and is compiled
independently from the existing binary target. The only operational follow-up
is publishing a new patch version because crates.io versions cannot be
replaced.

## Related Records

- [ADR-0030: Provide a documentation target for the CLI crate](../../../docs/adr/0030-docs-rs-library-target.md)
- [Release 0.3.0](../release-0-3-0/proposal.md)
