# Tasks: Release Harness-Gate 0.3.2

**Parent:** [proposal.md](proposal.md)
**Status:** Implemented; acceptance evidence reviewed against merged PR #47,
release workflow `33295289119`, and the published `0.3.2` crate and docs.rs
documentation.

- [x] Update package version, lockfile, CLI version assertion, changelog, and
  documented release links.
- [x] Run package validation, formatting, linting, and the full test suite.
- [x] Create the release commit and verify all required CI checks.
- [x] Tag `v0.3.2` and verify GitHub Release assets and workflow success -
  [GitHub Release v0.3.2](https://github.com/musutrade/Harness-Gate/releases/tag/v0.3.2)
  has four assets and workflow `33295289119` passed all jobs.
- [x] Verify crates.io publication and docs.rs build for `0.3.2` -
  [`cargo info harness-gate@0.3.2`](https://crates.io/crates/harness-gate/0.3.2)
  resolves the published crate, and [docs.rs documentation for
  `0.3.2`](https://docs.rs/harness-gate/0.3.2/harness_gate/) is available.
