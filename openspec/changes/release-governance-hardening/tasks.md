# Tasks: Release Governance Hardening

**Parent:** [proposal.md](proposal.md), [design.md](design.md),
[release-governance specification](specs/release-governance/spec.md), and
[ADR-0035](../../../docs/adr/0035-protect-release-eligibility.md)
**Status:** Implemented; acceptance evidence reviewed against PR #66 and CI
run `33478957797` on 2026-09-01. G-02 production release evidence is now
closed by the immutable `v0.3.5` release below.

- [x] **1.1 (P0, M)** Implement strict tag, package version, tag/commit, and
  protected-main ancestry validation.
  **Acceptance:** Focused fixtures reject malformed, mismatched, and non-main tags.
- [x] **1.2 (P0, M)** Bind eligibility to the exact successful `main` push CI
  run and `Required Quality Aggregate` job.
  **Acceptance:** Pull-request, wrong-commit, failed, missing, or ambiguous
  evidence blocks publication; the aggregate itself includes release policy
  and inventory contract tests.
- [x] **1.3 (P0, S)** Make build and quality jobs depend on policy and retain the
  policy JSON artifact.
  **Acceptance:** No build or publication job starts after policy failure.
- [x] **2.1 (P0, S)** Configure the immutable `v*` tag ruleset.
  **Acceptance:** Update and deletion are active with no bypass actor.
- [x] **2.2 (P0, S)** Configure the protected `release` environment and `v*`
  tag-only deployment policy.
  **Acceptance:** Publication jobs require explicit review and administrators
  cannot bypass the environment.
- [x] **2.3 (P0, S)** Attach the environment to GitHub Release and crates.io jobs.
  **Acceptance:** Publication credentials are unavailable before environment approval.
- [x] **3.1 (P0, S)** Pass focused tests, strict OpenSpec validation,
  documentation consistency, pull-request CI, and merged `main` CI.
  **Acceptance:** ADR/OpenSpec become Accepted/Implemented only after green evidence.

## Local Evidence

The 20 release-tool tests, strict OpenSpec validation, documentation
consistency, workflow YAML parsing, and diff checks pass locally. A read-only
end-to-end policy run against `main` commit `0f3b4c8` selected CI run
`33475502227` and its successful `Required Quality Aggregate` job. Repository
tag ruleset [21989651](https://github.com/musutrade/Harness-Gate/rules/21989651)
is active with no bypass actors; environment `release` (`20983060444`) disables
administrator bypass, requires reviewer `higoalespn`, and accepts only tag
deployment policy `v*` (`58783819`).
PR [#66](https://github.com/musutrade/Harness-Gate/pull/66) merged after 20
required checks passed in CI run
`33478957797`, including `Release Governance Contracts` and
`Required Quality Aggregate`. This closes R-10 implementation.

## Production Evidence (G-02)

The next immutable release requirement was exercised successfully:

- PR [#71](https://github.com/musutrade/Harness-Gate/pull/71) merged as
  `190cfa85699231591e3f74612e38156f6a102ef9`.
- Exact merged-commit `main` CI run
  [33524026442](https://github.com/musutrade/Harness-Gate/actions/runs/33524026442)
  passed all jobs, including `Required Quality Aggregate`.
- Tag [`v0.3.5`](https://github.com/musutrade/Harness-Gate/releases/tag/v0.3.5)
  was created at that commit. Release run
  [33525736285](https://github.com/musutrade/Harness-Gate/actions/runs/33525736285)
  passed eligibility, builds, quality, signing, provenance, GitHub Release,
  and crates.io publication after manual `release` environment approval.
