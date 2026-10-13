# Proposal: Harden Release Governance

**Status:** Implemented
**Date:** 2026-09-01

## Goals

- Require every release tag to match the package version and resolve to a
  commit in protected `main`.
- Require a successful full `main` push CI run and
  `Required Quality Aggregate` for the exact tagged commit.
- Make `v*` tags immutable after creation.
- Put GitHub Release and crates.io credentials behind a protected `release`
  environment restricted to version tags.
- Retain machine-readable release eligibility evidence.

## Non-goals

- Do not create a new package version or perform the G-02 production release.
- Do not replace release inventory, checksum, Sigstore, SBOM, or provenance
  verification.
- Do not refactor the complete CI workflow into a reusable workflow.
- Do not claim independent separation of duties while one maintainer owns the
  repository.

## Success Metrics

| Boundary | Success criterion |
| --- | --- |
| Tag identity | Invalid SemVer, package mismatch, or tag/commit mismatch fails before builds start. |
| Protected source | A tag whose commit is not reachable from `origin/main` fails. |
| Quality chain | Publication requires a successful exact-commit `main` push CI and aggregate job. |
| Tag governance | `refs/tags/v*` cannot be updated or deleted through normal or administrator actions. |
| Credential boundary | Both publication jobs wait on the protected `release` environment and accept only `v*` tags. |
| Evidence | The policy job retains repository, tag, commit, CI run, and aggregate job identities. |

## Risk Assessment

**Risk: Medium.** The change is fail closed and affects only tag-triggered
publication, but a GitHub API, ruleset, environment, or naming mismatch can
block a legitimate release. Focused offline fixtures cover policy decisions;
the next real tag release supplies G-02 operational evidence.

## Related Records

- [ADR-0035: Protect Release Eligibility and Publication](../../../docs/adr/0035-protect-release-eligibility.md)
- [ADR-0034: Enforce Fail-Closed Trust Boundaries](../../../docs/adr/0034-fail-closed-trust-boundaries.md)
- [Release governance operations](../../../docs/release-governance.md)
