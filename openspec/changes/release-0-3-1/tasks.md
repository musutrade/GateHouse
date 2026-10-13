# Tasks: Release Harness-Gate 0.3.1

- [x] Update package version, lockfile, CLI version assertion, changelog, and
  documented release links.
- [x] Run package validation, formatting, linting, and the full test suite.
- [x] Create the release PR and verify all required CI checks - PR [#44](https://github.com/musutrade/Harness-Gate/pull/44) passed all required checks in workflow [33232713161](https://github.com/musutrade/Harness-Gate/actions/runs/33232713161).
- [x] Merge the release PR and tag `v0.3.1` - merged as `1cfa136` and tag `v0.3.1` triggered workflow [33233044134](https://github.com/musutrade/Harness-Gate/actions/runs/33233044134).
- [x] Verify GitHub Release assets, crates.io publication, and docs.rs build - [GitHub Release v0.3.1](https://github.com/musutrade/Harness-Gate/releases/tag/v0.3.1) has four assets, `cargo info harness-gate@0.3.1` resolves, and docs.rs [Build #4249325](https://docs.rs/crate/harness-gate/0.3.1/builds/4249325) reports all builds succeeded.
