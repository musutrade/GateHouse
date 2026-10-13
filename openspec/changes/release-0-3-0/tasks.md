# Tasks: Release Harness-Gate 0.3.0

- [x] Update package version, lockfile, CLI version assertion, changelog, and
  documented release links (P0, S) - `harness-gate` is `0.3.0` in Cargo,
  lockfile, CLI output, changelog, and README download examples.
- [x] Run package validation, formatting, linting, and the full test suite (P0, M)
  - `cargo check`, `cargo package`, `cargo fmt -- --check`, the version contract
  test, strict OpenSpec validation, and the PR CI suite passed.
- [x] Create the release PR and verify all required CI checks (P0, M) - PR
  [#41](https://github.com/musutrade/Harness-Gate/pull/41) passed all 17 CI
  checks before merge.
- [x] Merge the release PR and tag `v0.3.0` (P0, S) - merged as `a1a8c85` and
  pushed as tag `v0.3.0`.
- [x] Verify the GitHub Release assets and crates.io publication (P0, M) -
  [GitHub Release v0.3.0](https://github.com/musutrade/Harness-Gate/releases/tag/v0.3.0)
  has four platform assets; [crates.io](https://crates.io/crates/harness-gate/0.3.0)
  and `cargo info harness-gate@0.3.0` report version `0.3.0`. Release workflow
  [run 33229902787](https://github.com/musutrade/Harness-Gate/actions/runs/33229902787)
  passed all five jobs.
