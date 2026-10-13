# Proposal: Release Harness-Gate 0.3.0

## Why

The repository has accumulated the configuration, verification-plan, scheduling,
quality-gate, and project-scoped configuration work since the `0.2.0` release.
The package and downloadable binaries should expose that merged functionality as
one versioned release.

## Goals

- Publish `harness-gate` version `0.3.0` to crates.io.
- Build and publish GitHub Release `v0.3.0` for the supported platforms.
- Synchronize package metadata, changelog, CLI version output, and documented
  download links.

## Non-goals

- No runtime behavior changes beyond the already merged `main` commits.
- No changes to release targets or the release workflow.

## Success Metrics

- Cargo package validation and the full CI suite pass for `0.3.0`.
- Tag `v0.3.0` triggers a successful release workflow.
- crates.io and GitHub Releases both expose `0.3.0` with the expected assets.

## Risk Assessment

Low. The version is greater than the published `0.2.0`; Cargo and crates.io
reject duplicate versions. A follow-up patch release can correct release
metadata without rewriting the published artifact.
