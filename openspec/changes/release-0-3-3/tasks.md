# Tasks: Release Harness-Gate 0.3.3

**Parent:** [proposal.md](proposal.md)
**Status:** Implemented; acceptance evidence reviewed against merged PR #52,
release workflow `33315285211`, and the published `0.3.3` crate and docs.rs
documentation.

- [x] Correct README anchors and package-safe GitHub documentation links.
- [x] Expand docs.rs crate-level documentation and add explicit docs.rs
  metadata.
- [x] Update package version, lockfile, CLI version assertion, snapshot,
  changelog, and repository documentation URLs.
- [x] Verify strict Clippy, `cargo +nightly udeps`, package contents, and CLI
  contract output locally.
- [x] Create and pass the release PR CI checks - PR [#52](https://github.com/musutrade/Harness-Gate/pull/52)
  merged as `95678d93ac0815e37e5ab52f3f6a84c97729f85c` after all required checks
  passed in [CI run 33314576672](https://github.com/musutrade/Harness-Gate/actions/runs/33314576672).
- [x] Tag `v0.3.3`, publish the GitHub Release assets, and verify the crates.io
  and docs.rs pages - [GitHub Release v0.3.3](https://github.com/musutrade/Harness-Gate/releases/tag/v0.3.3)
  contains four platform assets and [release workflow 33315285211](https://github.com/musutrade/Harness-Gate/actions/runs/33315285211)
  passed all jobs; [crates.io 0.3.3](https://crates.io/crates/harness-gate/0.3.3)
  is published and docs.rs [build 4264644](https://docs.rs/crate/harness-gate/0.3.3/builds/4264644)
  reports all builds succeeded with the documentation available at
  [docs.rs/harness-gate/0.3.3/harness_gate](https://docs.rs/harness-gate/0.3.3/harness_gate/).
