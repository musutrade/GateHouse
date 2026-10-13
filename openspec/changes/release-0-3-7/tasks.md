# Tasks: Release Harness-Gate 0.3.7

**Parent:** [proposal.md](proposal.md)
**Status:** Implemented; final release evidence is recorded in
[proposal.md](proposal.md).

- [x] Synchronize package version, lockfile, CLI version assertion, snapshots,
  changelog, project status, README examples, and release governance.
- [x] Run package validation, formatting, linting, tests, OpenSpec validation,
  and documentation consistency checks locally.
- [x] Create the release pull request and pass every required PR check -
  [PR #81](https://github.com/musutrade/Harness-Gate/pull/81).
- [x] Merge the release pull request, wait for the protected `main` CI run,
  and create the immutable `v0.3.7` tag at that exact commit.
- [x] Approve the protected publication environment after policy and quality
  gates pass; verify the release workflow publishes the exact asset inventory
  to GitHub Release and the crate to crates.io.
- [x] Download and verify the published inventory, checksums, Sigstore
  signatures/certificates, provenance, crate, and clean-environment installer
  flow.
- [x] Record the tag, workflow, release URL, asset verification, and clean
  environment results in the release proposal and release governance, then
  mark this release record Implemented.
