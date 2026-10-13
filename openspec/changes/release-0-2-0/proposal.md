# Proposal: Release Harness-Gate 0.2.0

## Why

The terminal feedback enhancement is merged into `main`, but the package and binary distribution remain at `0.1.0`. A versioned release is required to publish the merged functionality to GitHub Releases and crates.io.

## Goals

- Publish `harness-gate` version `0.2.0` to crates.io.
- Build GitHub Release `v0.2.0` for supported platforms.
- Make release failures visible instead of silently ignored.

## Non-goals

- Changing runtime behavior beyond the already merged terminal feedback enhancement.
- Rewriting the release pipeline or adding new target platforms.

## Success Metrics

- `Cargo.toml`, `Cargo.lock`, CLI version output, changelog, and download links identify `0.2.0`.
- Tag `v0.2.0` triggers a successful release workflow.
- GitHub Release and crates.io both expose `0.2.0`.

## Risk Assessment

Low. Cargo rejects duplicate package versions on crates.io, and the workflow is tag-triggered and reversible by publishing a follow-up patch release if needed.
