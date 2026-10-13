# Proposal: Update Dependencies to Latest Versions

## Why

The project's dependencies are currently pinned to versions from early 2024-2025. Updating to the latest stable versions will bring security patches, performance improvements, bug fixes, and access to new features. This maintenance work reduces technical debt and ensures the project benefits from upstream improvements.

## What Changes

Update all Rust dependencies in `tools/harness-gate/Cargo.toml` to their latest compatible versions:

### Major Dependencies to Update
- `anyhow`: 1.0.x → latest 1.0.x
- `serde`: 1.0.x → latest 1.0.x  
- `serde_json`: 1.0.x → latest 1.0.x
- `toml`: 1.1.x → latest (check for breaking changes if moving to 2.x)
- `rayon`: 1.12.x → latest 1.x
- `ignore`: 0.4.x → latest 0.4.x
- `libc`: 0.2.x → latest 0.2.x
- `regex`: 1.13.x → latest 1.x
- `globset`: 0.4.x → latest 0.4.x
- `clap`: 4.6.x → latest 4.x
- `chrono`: 0.4.x → latest 0.4.x
- `url`: 2.5.x → latest 2.x

### Dev Dependencies
- `tempfile`: 3.8.x → latest 3.x

### Actions
- Run `cargo update` to update Cargo.lock within semver constraints
- Check for any deprecated API usage that needs updating
- Update version numbers in Cargo.toml if minor/patch updates are available
- Verify all tests pass after updates
- Check for breaking changes in changelog for any major version bumps

## Goals

- Update all dependencies to latest stable versions within semver compatibility
- Maintain 100% test passing rate
- No breaking changes to harness-gate's public API
- Document any behavioral changes from dependency updates

## Non-Goals

- Migrating to different dependencies (e.g., replacing `clap` with another CLI framework)
- Major version upgrades that require significant code changes (defer to separate change)
- Adding new dependencies

## Success Metrics

- All dependencies updated to latest compatible versions
- `cargo build` succeeds without warnings
- `cargo test` passes 100%
- `cargo clippy` reports zero warnings
- CI pipeline passes on all platforms (Linux, macOS, Windows)
- Binary size remains similar (±5%)
- No performance regressions in test execution time

## Capabilities

### New Capabilities
<!-- None - this is a maintenance change -->

### Modified Capabilities
<!-- None - dependency updates should not change requirements. If they do, this needs
     deeper investigation. Set skip_specs: true in .openspec.yaml since this is pure
     dependency maintenance. -->

## Impact

### Affected Areas
- Build system: Cargo.toml and Cargo.lock
- All modules: dependency version updates may have subtle API changes
- CI pipeline: May need additional time for first build with new dependencies

### Risk Assessment
- **Low risk**: Semver-compatible updates should be safe
- **Medium risk**: Deprecated API usage may surface warnings
- **Low risk**: Performance impact expected to be neutral or positive

### Rollback Plan
- Revert Cargo.toml and Cargo.lock to previous commit
- All changes are in version control and easily reversible
