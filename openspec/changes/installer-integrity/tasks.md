# Tasks: Installer Artifact Verification

**Parent:** [proposal.md](proposal.md), [design.md](design.md),
[installer specification](specs/installer-integrity/spec.md), and
[ADR-0036](../../../docs/adr/0036-verify-installer-artifacts.md)
**Status:** Implemented; PR #68 and merged-main CI run 33495394825 provide
acceptance evidence. G-02 real-release compatibility remains a separate
follow-up.

- [x] **1.1 (P0, M)** Require explicit strict SemVer and immutable tag URLs.
  **Acceptance:** Missing, malformed, prerelease-leading-zero, and unsupported
  inputs fail before network access.
- [x] **1.2 (P0, M)** Download and verify checksum and Sigstore metadata.
  **Acceptance:** HTTP, empty, missing-entry, digest, issuer, workflow, and
  tag mismatches block installation.
- [x] **1.3 (P0, M)** Add confined temporary downloads and atomic destination
  publication.
  **Acceptance:** Existing target remains unchanged on failure; symlink and
  unsafe-permission paths are rejected.
- [x] **1.4 (P0, S)** Apply the same immutable-tag boundary to source mode.
  **Acceptance:** Source mode clones only the requested tag and publishes via
  the atomic destination path.
- [x] **2.1 (P0, M)** Add offline shell contract fixtures and run them in the
  required CI aggregate.
  **Acceptance:** Success, checksum, signature, target symlink, parent symlink,
  and cleanup cases pass without network access.
- [x] **2.2 (P0, S)** Update ADR/OpenSpec status after merged PR and main CI.
  **Acceptance:** Acceptance evidence links PR, CI run, and future real-release
  compatibility note.

## Acceptance Evidence

PR [#67](https://github.com/musutrade/Harness-Gate/pull/67) merged the initial
implementation as `2976611`; PR [#68](https://github.com/musutrade/Harness-Gate/pull/68)
closed the follow-up gaps and passed its 20-check CI run `33494073660` before
merging as `ec65742`. The merged `main` commit passed all 20 checks in CI run
`33495394825`, including `Required Quality Aggregate`.
