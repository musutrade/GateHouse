# Tasks: Release Harness-Gate 0.2.0

- [x] Update package version and version assertion (P0, S)
- [x] Update changelog and installation links (P1, S)
- [x] Make crates.io publish failures fail the workflow (P0, S)
- [x] Run package validation and full test suite (P0, M) - `cargo package --allow-dirty`, formatting, Clippy, and 83 tests pass.
- [x] Commit, push, and tag `v0.2.0` (P0, S)
- [x] Verify GitHub Release assets and crates.io publication (P0, M) - Release workflow succeeded; four platform assets uploaded and crates.io reports `0.2.0`.
