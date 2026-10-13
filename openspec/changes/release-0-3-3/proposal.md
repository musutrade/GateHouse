# Proposal: Release Harness-Gate 0.3.3

**Status:** Implemented

## Why

The published README is also rendered by crates.io, where Cargo rewrites
relative links against the package root. Links to the language README, local
documentation, the license, and contribution guide therefore pointed at paths
that do not exist in the packaged crate. Several navigation anchors also did
not match their headings on GitHub. A patch release is required because
crates.io versions are immutable.

## Goals

- Make README navigation anchors resolve to existing headings.
- Use stable GitHub URLs for repository documentation from GitHub, crates.io,
  and docs.rs.
- Expand the documentation-only library target shown on docs.rs.
- Publish `harness-gate 0.3.3` and GitHub Release `v0.3.3`.

## Non-goals

- No public runtime library API.
- No changes to the CLI behavior or configuration schema.
- No removal of imports that are already proven used by strict Clippy and
  `cargo +nightly udeps`.

## Success Metrics

- `cargo package --locked --allow-dirty --no-verify` contains the corrected
  README and `src/lib.rs`.
- Format, strict Clippy, tests, CLI contracts, docs consistency, and strict
  OpenSpec validation pass.
- crates.io and docs.rs expose version `0.3.3` with the updated links.
- GitHub Release `v0.3.3` contains all platform binaries.

## Acceptance Evidence

- PR [#52](https://github.com/musutrade/Harness-Gate/pull/52) merged as
  `95678d93ac0815e37e5ab52f3f6a84c97729f85c` after all required checks passed
  in [CI run 33314576672](https://github.com/musutrade/Harness-Gate/actions/runs/33314576672).
- Tag `v0.3.3` published the four platform assets in the
  [GitHub Release](https://github.com/musutrade/Harness-Gate/releases/tag/v0.3.3);
  the [release workflow 33315285211](https://github.com/musutrade/Harness-Gate/actions/runs/33315285211)
  completed successfully.
- [harness-gate 0.3.3 on crates.io](https://crates.io/crates/harness-gate/0.3.3)
  is published with the corrected README links.
- [docs.rs 0.3.3 documentation](https://docs.rs/harness-gate/0.3.3/harness_gate/)
  is available; [build 4264644](https://docs.rs/crate/harness-gate/0.3.3/builds/4264644)
  reports that all builds succeeded.

## Risk Assessment

**Risk: Low.** The release only changes documentation, package metadata, and
the version; the binary implementation remains unchanged.

## Related Records

- [ADR-0030: docs.rs library target](../../../docs/adr/0030-docs-rs-library-target.md)
- [ADR-0031: Harden gate boundaries](../../../docs/adr/0031-harden-gate-boundaries.md)
