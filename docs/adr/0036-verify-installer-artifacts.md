# ADR-0036: Verify Installer Artifacts Before Atomic Installation

## Status

**Accepted** (2026-09-01)

## Context

The original `install.sh` discovered `releases/latest` through an ad-hoc API
`grep`, downloaded directly to a predictable working-tree filename, and moved
that file without checking a checksum or signing identity. A failed HTTP
response, a mutable tag, a substituted output symlink, or a compromised
release asset could therefore reach a user's executable path.

The release workflow now produces explicit checksums and Sigstore bundles, and
the repository makes version tags immutable. The installer must consume those
trust signals rather than recreate a weaker latest-release path.

## Decision

`install.sh` requires an exact `v`-prefixed SemVer version (from
`--version` or `HARNESS_GATE_VERSION`) and downloads only from that tag. The
binary, `SHA256SUMS`, the manifest signature/certificate, and the binary
signature/certificate are written under a private temporary directory.

Before installation it:

- uses failure-reporting HTTPS downloads with retries and rejects empty files;
- verifies the binary's single manifest entry with SHA-256;
- verifies both the manifest and binary with `cosign verify-blob`, requiring
  the GitHub Actions issuer and the exact repository workflow/tag identity;
- rejects absolute, traversal, dot, newline, parent-component, directory, or
  symlink installation paths and group/other-writable destination directories;
  and
- copies to a create-new sibling in the destination directory and renames it
  over only a regular target after all verification succeeds.

Source installation also clones the immutable tag, builds into a temporary
Cargo root, and uses the same atomic destination publication boundary. It does
not execute a mutable remote script or silently select the latest version.

The installer is intentionally a Bash shell boundary (invoked as `bash
install.sh`); on Windows it is supported through MSYS2, Git Bash, or Cygwin.
Its path checks reject
pre-existing symlink components and targets; protection against a same-user
concurrent substitution remains dependent on the host filesystem's rename
semantics and should be documented for deployments that need stronger claims.

## Consequences

### Positive

- A network error, HTML error page, checksum mutation, signature mutation, or
  wrong OIDC identity leaves the existing installation unchanged.
- Version selection and the downloaded script can be reviewed against one
  immutable tag.
- Temporary files and partial downloads do not appear in the destination path.
- The same source-tag semantics apply to binary and source installation.

### Negative

- Users must choose a version and install `cosign`; old releases without the
  generated metadata cannot be installed by the binary path.
- The shell implementation needs platform-specific `stat` and hash-tool
  fallbacks and cannot provide directory-handle guarantees on every OS.
- A new release is required when users want to upgrade; there is no implicit
  `latest` channel.

## Alternatives Considered

- **Keep `releases/latest`:** rejected because it is mutable and cannot bind a
  download to a reviewed source identity.
- **Verify only SHA-256:** rejected because a compromised manifest could name a
  substituted binary; the keyless certificate also binds workflow and tag.
- **Install directly over the target:** rejected because failed or partial
  downloads could corrupt an existing executable and symlink targets are
  unsafe.
- **Recommend `raw/main | bash`:** rejected because the script itself would be
  mutable and outside the release verification chain.

## Rollback

If a verifier defect blocks installation, fix it in a pull request and publish
a new immutable tag. Existing binaries remain untouched. Never restore an old
tag by moving the tag reference; users can explicitly select an older valid
version whose metadata is still available.

## Related Records

- [ADR-0035: Protect Release Eligibility and Publication](0035-protect-release-eligibility.md)
- [ADR-0034: Enforce Fail-Closed Trust Boundaries](0034-fail-closed-trust-boundaries.md)
- [Installer integrity OpenSpec](../../openspec/changes/installer-integrity/proposal.md)
- [Release governance operations](../release-governance.md)

## Acceptance Evidence

- PR [#67](https://github.com/musutrade/Harness-Gate/pull/67) introduced the
  immutable-tag installer boundary and merged as `2976611`.
- PR [#68](https://github.com/musutrade/Harness-Gate/pull/68) closed the
  Windows asset, exact source-tag, identity-fixture, and signal-cleanup gaps;
  its 20-check CI run
  [33494073660](https://github.com/musutrade/Harness-Gate/actions/runs/33494073660)
  passed and it merged as `ec65742`.
- The merged `main` commit `ec65742c71f389ca6519ed34fdf7a736091356d2` passed
  all 20 checks, including `Required Quality Aggregate`, in
  [CI run 33495394825](https://github.com/musutrade/Harness-Gate/actions/runs/33495394825).

These checks close the implementation and offline-contract acceptance. G-02
still requires a future immutable tag's published SBOM, checksums, Sigstore,
provenance, and clean-environment offline consumer verification.
