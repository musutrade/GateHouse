# Proposal: Release Harness-Gate 0.3.7

**Status:** Implemented (2026-09-03)
**Date:** 2026-09-03

## Why

The previous published version is 0.3.6. Since that tag, `main` has accepted
the post-remediation hardening umbrella (ADR-0038 and the companion OpenSpec),
recorded the R-07 adapter-isolation wording inventory and OS-sandbox decision
matrix, added reproducible R-16/R-17 benchmark and allocation evidence, and
stabilized the Windows lease heartbeat lifecycle test. This patch publishes
those synchronized records and the stabilized quality baseline.

## Goals

- Publish package metadata, CLI output, generated contracts, snapshots,
  changelog, project status, and user-facing installation examples as version
  0.3.7.
- Keep the release workflow protected by exact main CI, immutable tags,
  signed inventory, provenance, and the `release` environment.
- Preserve explicit boundaries for work that is not part of this release.

## Non-goals

- No runtime CLI behavior, configuration schema, public API, or trust-boundary
  changes beyond the accepted v0.3.6 implementation.
- No OS-level adapter sandbox, DevRail staging G-03/G-04 acceptance,
  shadow/canary approval, or rollback authority.
- No mutation or reuse of existing version tags.

## Release contents

- Post-remediation hardening OpenSpec and ADR-0038 accepted records.
- R-07 adapter isolation wording inventory and OS-sandbox decision matrix with
  documentation consistency fixtures.
- R-16 scheduler/wait scenario evidence and R-17 validation allocation
  evidence under `docs/benchmarks`.
- Windows lease heartbeat lifecycle test stabilization.
- Version, changelog, README, project status, and release documentation
  synchronization.

## Acceptance

Before publication, the release PR must pass locked tests, formatting,
strict Clippy, dependency audit, schema generation, documentation
consistency, quality-script tests, release tests, and package validation.
After merge, tag v0.3.7 only on the exact protected main commit and approve
the protected release environment only after the release policy and build
matrix pass. Record the final PR, commit, workflow, release URL, asset
inventory, and crate URL here before marking this record Implemented.

## Related records

- [ADR-0035: Protect Release Eligibility and Publication](../../../docs/adr/0035-protect-release-eligibility.md)
- [ADR-0038: Post-Remediation Hardening](../../../docs/adr/0038-post-remediation-hardening.md)
- [Post-remediation hardening OpenSpec](../post-remediation-hardening/proposal.md)
- [Release governance operations](../../../docs/release-governance.md)

## Acceptance Evidence

- Pull request [#81](https://github.com/musutrade/Harness-Gate/pull/81) merged
  as `0491ecca098bfd6d48dfc17829f700a69734a996` after all 21 required checks
  passed in [PR CI run 33749197449](https://github.com/musutrade/Harness-Gate/actions/runs/33749197449).
- The exact merge commit passed protected `main` CI in
  [run 33750527365](https://github.com/musutrade/Harness-Gate/actions/runs/33750527365),
  including `Required Quality Aggregate`.
- The immutable [`v0.3.7` tag](https://github.com/musutrade/Harness-Gate/releases/tag/v0.3.7)
  resolves to that same commit. Release workflow
  [33751905950](https://github.com/musutrade/Harness-Gate/actions/runs/33751905950)
  passed eligibility, all four platform builds, release quality gates,
  Sigstore signing, provenance, GitHub Release creation, and crates.io
  publication after protected `release` environment approvals.
- The published [GitHub Release](https://github.com/musutrade/Harness-Gate/releases/tag/v0.3.7)
  contains the exact 21-file upload set declared by `release-inventory.json`.
  A clean download was verified with `release_inventory.py verify` and
  `sha256sum -c SHA256SUMS`.
- The public [harness-gate 0.3.7 crate](https://crates.io/crates/harness-gate/0.3.7)
  is available; the downloaded archive reports `version = "0.3.7"` and hashes
  to `c1ab26480ee179c72370f9fd8c30860ca401fd0df54ccbcf7875496ba90df7cf`.
- The release record does not expand the product boundary: R-07 OS-level
  sandboxing, DevRail staging G-03/G-04, shadow/canary approval, and rollback
  authority remain external or future work and are not claimed as accepted.
