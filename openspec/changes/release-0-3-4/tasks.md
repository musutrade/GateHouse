# Tasks: Release Harness-Gate 0.3.4

**Parent:** [proposal.md](proposal.md)
**Status:** Superseded after the protected publication attempt failed closed;
recovery is tracked by [release-0-3-5](../release-0-3-5/tasks.md).

- [x] Synchronize package metadata, lockfile, CLI version assertion, CLI
  snapshot, changelog, project status, and documented release commands.
- [x] Run package validation, formatting, linting, tests, OpenSpec validation,
  and documentation consistency checks locally.
- [x] Create the release pull request and pass every required PR check -
  [PR #70](https://github.com/musutrade/Harness-Gate/pull/70).
- [x] Merge the release pull request, wait for the protected `main` CI run,
  and create the immutable `v0.3.4` tag at that exact commit.
- [ ] Approve the protected publication environment after policy and quality
  gates pass; verify the release workflow completes without retries that alter
  the source commit - approval completed, but
  [release run 33504911061](https://github.com/musutrade/Harness-Gate/actions/runs/33504911061)
  failed exact inventory verification before publication.
- [ ] Download the published asset set and verify the exact inventory,
  checksums, Sigstore signatures/certificates, and GitHub provenance - no
  release asset set was published.
- [ ] Run the documented binary and source installer flows in a clean,
  network-isolated environment and retain their output as release evidence -
  moved to `v0.3.5`.
- [ ] Record the tag, workflow, release URL, asset verification, and clean
  environment results in ADR-0032 and the DevRail capability OpenSpec, then
  mark this release record implemented - moved to `v0.3.5`.
