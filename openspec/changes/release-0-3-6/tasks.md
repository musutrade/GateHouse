# Tasks: Release Harness-Gate 0.3.6

**Parent:** [proposal.md](proposal.md)
**Status:** Implemented; final release evidence is recorded in
[proposal.md](proposal.md).

- [x] Add the English configuration reference and preserve the Chinese
  reference at docs/configuration.zh-CN.md.
- [x] Add the English JSON Schema catalog and enforce documentation
  language/schema presence in the docs-consistency check.
- [x] Synchronize package version, lockfile, CLI assertion, snapshots,
  changelog, project status, README examples, and release governance.
- [x] Run focused and full local release validation.
- [x] Create the release pull request and pass every required PR check.
- [x] Merge the pull request, wait for protected main CI, and create the
  immutable v0.3.6 tag at that exact commit.
- [x] Approve the protected release environment and verify publication.
- [x] Verify the published inventory, checksums, signatures, provenance,
  crate, and clean-environment installer flow.
- [x] Record final release evidence and mark this release record Implemented.
