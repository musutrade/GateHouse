# Proposal: Release Harness-Gate 0.3.6

**Status:** Implemented (2026-09-03)
**Date:** 2026-09-02

## Why

The previous published version is 0.3.5. The current source contains the
reviewed post-remediation implementation for selected R-13 through R-18
tracks, but the public documentation still exposed the configuration
reference only in Chinese. This release records the implementation and
publishes a complete English configuration and JSON Schema reference.

## Goals

- Publish package metadata, CLI output, generated contracts, snapshots,
  changelog, and user-facing installation examples as version 0.3.6.
- Make docs/configuration.md the English schema v2 reference and retain the
  existing Chinese content in docs/configuration.zh-CN.md.
- Add schema/README.md as an English catalog for every shipped JSON Schema.
- Keep the release workflow protected by exact main CI, immutable tags,
  signed inventory, provenance, and the release environment.
- Preserve explicit boundaries for work that is not part of this release.

## Non-goals and open boundaries

- No DevRail business-code or policy change.
- No claim of DevRail staging G-03/G-04 acceptance, shadow/canary approval,
  or rollback authority.
- No OS-level network, filesystem, resource, or complete descendant sandbox
  for adapters; R-07 remains a separate future decision.
- No claim that the post-remediation umbrella ADR/OpenSpec is complete while
  cross-platform and other unchecked acceptance evidence remains open.

## Release contents

- R-13 webhook host/address policy and redacted destination diagnostics.
- R-14 through R-17 lifecycle, typed failure, scheduler, runner, and
  configuration validation hardening present in the source tree.
- R-18 repository and quality governance files and checks.
- English and Chinese configuration references plus the JSON Schema catalog.

## Acceptance

Before publication, the release PR must pass locked tests, formatting,
strict Clippy, dependency audit, schema generation, documentation
consistency, quality-script tests, release tests, and package validation.
After merge, tag v0.3.6 only on the exact protected main commit and approve
the protected release environment only after the release policy and build
matrix pass. The final PR, commit, workflow, release URL, asset inventory,
crate URL, and consumer verification are recorded below.

## Related records

- ADR-0038: Plan Post-Remediation Security, Reliability, and Maintenance Follow-ups
- ADR-0035: Protect Release Eligibility and Publication
- Release governance: docs/release-governance.md
- Post-remediation hardening: ../post-remediation-hardening/proposal.md

## Acceptance Evidence

- Pull request [#75](https://github.com/musutrade/Harness-Gate/pull/75) merged
  as `6a9066b7f5dba241a3190a5508727cd21ba2c9b0` after all 21 required checks
  passed in [PR CI run 33701746324](https://github.com/musutrade/Harness-Gate/actions/runs/33701746324).
- The exact merge commit passed protected `main` CI in
  [run 33702793530](https://github.com/musutrade/Harness-Gate/actions/runs/33702793530),
  including the successful `Required Quality Aggregate` job
  [100488162319](https://github.com/musutrade/Harness-Gate/actions/runs/33702793530/job/100488162319).
  The immutable [`v0.3.6` tag](https://github.com/musutrade/Harness-Gate/releases/tag/v0.3.6)
  resolves to that same commit.
- Release workflow
  [33704014615](https://github.com/musutrade/Harness-Gate/actions/runs/33704014615)
  completed eligibility, four platform builds, release quality gates, signed
  inventory, Sigstore verification, provenance verification, GitHub Release
  creation, and crates.io publication after the protected `release` environment
  approval.
- The published [GitHub Release](https://github.com/musutrade/Harness-Gate/releases/tag/v0.3.6)
  contains the exact 21-file upload set declared by `release-inventory.json`.
  A clean temporary download was verified with `release_inventory.py verify` and
  `sha256sum -c SHA256SUMS`; every GitHub API asset digest matched the downloaded
  file. The release policy artifact records the exact tag, commit, main CI run,
  and aggregate job.
- The public [harness-gate 0.3.6 crate](https://crates.io/crates/harness-gate/0.3.6)
  is available; its downloaded archive SHA256 matches the crates.io API
  checksum. In isolated temporary directories, the v0.3.6 binary installer
  verified the published Sigstore certificates and installed a binary reporting
  `harness-gate 0.3.6`; the immutable-tag `--from-source` path did the same.
  The offline installer integrity contract also passed.
- The release record does not expand the product boundary: R-07 OS-level
  sandboxing, DevRail staging G-03/G-04, shadow/canary approval, and rollback
  authority remain external or future work and are not claimed as accepted.
