# ADR-0035: Protect Release Eligibility and Publication

## Status

**Accepted** (2026-09-01)

## Context

Harness-Gate publishes binaries, a CycloneDX SBOM, checksums, Sigstore
signatures, provenance, and the crate from a tag-triggered workflow. The
release inventory and privileged Actions are already fail closed, and `main`
is protected by an inherited organization ruleset that requires
`Required Quality Aggregate`.

The release trigger nevertheless accepts any `v*` ref. Before this decision it
did not prove that the tag resolved to the package version, that its commit was
in protected `main`, or that the exact commit had completed the full `main`
push CI workflow. Version tags also had no immutability ruleset, and the jobs
holding write, OIDC, attestation, and crates.io credentials did not enter a
protected GitHub environment.

These gaps let an incorrectly placed or mutable tag reach publication even
when the ordinary branch review path was healthy. Rerunning a smaller release
test subset is useful defense in depth, but it is not evidence that the exact
tagged commit passed the repository's complete cross-platform quality chain.

## Decision

Harness-Gate will enforce release governance at three independent boundaries.

### 1. Verify release eligibility before building

The first release job will fail closed unless all of the following are true:

- the ref is exact SemVer with a `v` prefix;
- the version after `v` exactly matches `tools/harness-gate/Cargo.toml`;
- the tag resolves to the workflow commit;
- the commit is reachable from `refs/remotes/origin/main`; and
- GitHub records a completed, successful `push` run of
  `.github/workflows/ci.yml` on `main` for that exact commit, including one
  successful `Required Quality Aggregate` job. The aggregate includes the
  release policy and inventory contract suite.

The job receives only `actions: read` and `contents: read`. It publishes a
machine-readable policy artifact. Build and release-quality jobs depend on
this check, and publication depends on all three.

### 2. Make version tags immutable

An active repository tag ruleset will match `refs/tags/v*` and prohibit update
and deletion without a bypass actor. Existing legacy version tags are covered
by the same immutability rule. Tag creation remains an explicit maintainer
operation; workflow policy determines whether a newly created tag is eligible
to publish.

### 3. Protect publication credentials

Both the signed GitHub Release job and the crates.io publication job will use
the `release` environment. That environment will:

- allow deployments only from tags matching `v*`;
- require explicit approval by the configured maintainer reviewer;
- disallow administrator bypass; and
- allow self-review only because the repository currently has one maintainer.

Adding a second maintainer or release team should be followed by disabling
self-review. Repository write, OIDC, attestation, and crates.io credentials are
not available to policy, build, or quality jobs.

## Consequences

### Positive

- A tag outside protected `main`, on the wrong package version, or without the
  exact successful `main` CI run cannot consume release credentials.
- Published version tags cannot be moved or deleted, so release identity stays
  stable after creation.
- Environment approval is a separate, auditable publication decision after
  automated eligibility and quality checks.
- The retained policy artifact links the release run to the exact branch CI
  and aggregate job used for authorization.

### Negative

- A release must wait for the tagged commit's `main` push CI and a manual
  environment approval.
- Renaming the CI workflow, default branch, aggregate job, or release
  environment requires a reviewed policy update.
- The current single-maintainer environment provides a deliberate second step,
  not separation of duties. A release team is required for independent review.
- GitHub Actions or environment availability can block a release even when the
  source itself is healthy.

## Alternatives Considered

- **Rerun only the release workflow's quality job:** rejected because it does
  not prove the exact commit passed the complete cross-platform `main` chain.
- **Trust that maintainers create tags correctly:** rejected because tag
  placement and package version are machine-verifiable authorization inputs.
- **Use only an environment approval:** rejected because a reviewer should not
  have to manually reconstruct ancestry, version, and CI evidence.
- **Restrict tag creation but allow later mutation by administrators:** rejected
  because published release identity must remain immutable.

## Rollback

If the policy has a false positive, revert the workflow and policy script by
pull request while retaining the tag ruleset and environment. If GitHub
governance itself is unavailable, an administrator may deactivate the
environment or ruleset only through an audited repository-setting change; a
published tag must never be moved as a rollback mechanism. Failed release runs
and policy artifacts are retained for diagnosis.

## Related Records

- [ADR-0034: Enforce Fail-Closed Trust Boundaries](0034-fail-closed-trust-boundaries.md)
- [ADR-0032: Harness-Gate and DevRail Capability Contracts](0032-harness-gate-devrail-capability-contracts.md)
- [Release governance OpenSpec](../../openspec/changes/release-governance-hardening/proposal.md)
- [Release governance operations](../release-governance.md)

## Acceptance Evidence

- PR [#66](https://github.com/musutrade/Harness-Gate/pull/66) merged after all
  required checks passed.
- CI run
  [33478957797](https://github.com/musutrade/Harness-Gate/actions/runs/33478957797)
  passed 20 checks, including the release governance contract suite and
  `Required Quality Aggregate`.
- Repository tag ruleset
  [21989651](https://github.com/musutrade/Harness-Gate/rules/21989651) and
  protected `release` environment `20983060444` were read back from the
  GitHub API with the expected active/no-bypass/tag-only settings.
- A read-only policy run against `main` commit `0f3b4c8` selected the successful
  exact-commit CI run `33475502227` and aggregate job `99756631876`.
- PR [#71](https://github.com/musutrade/Harness-Gate/pull/71) merged the
  corrected publication workflow as `190cfa85699231591e3f74612e38156f6a102ef9`.
  The exact-commit protected `main` run
  [33524026442](https://github.com/musutrade/Harness-Gate/actions/runs/33524026442)
  passed all jobs and `Required Quality Aggregate`.
- The immutable [`v0.3.5` release](https://github.com/musutrade/Harness-Gate/releases/tag/v0.3.5)
  completed in run
  [33525736285](https://github.com/musutrade/Harness-Gate/actions/runs/33525736285)
  after explicit `release` environment approval; no tag mutation or bypass was
  used.
- The subsequent immutable [`v0.3.6` release](https://github.com/musutrade/Harness-Gate/releases/tag/v0.3.6)
  points to merge commit `6a9066b7f5dba241a3190a5508727cd21ba2c9b0`. Protected
  `main` CI run
  [33702793530](https://github.com/musutrade/Harness-Gate/actions/runs/33702793530)
  and release workflow
  [33704014615](https://github.com/musutrade/Harness-Gate/actions/runs/33704014615)
  passed all eligibility, quality, signing, provenance, and publication jobs;
  no tag mutation or bypass was used.
- The subsequent immutable [`v0.3.7` release](https://github.com/musutrade/Harness-Gate/releases/tag/v0.3.7)
  points to merge commit `0491ecca098bfd6d48dfc17829f700a69734a996`. Protected
  `main` CI run
  [33750527365](https://github.com/musutrade/Harness-Gate/actions/runs/33750527365)
  and release workflow
  [33751905950](https://github.com/musutrade/Harness-Gate/actions/runs/33751905950)
  passed all eligibility, quality, signing, provenance, and publication jobs;
  no tag mutation or bypass was used.
