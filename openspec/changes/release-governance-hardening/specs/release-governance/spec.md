# Release Governance

## ADDED Requirements

### Requirement: Version tags bind to reviewed source

The release workflow SHALL reject a tag unless it is exact `v`-prefixed SemVer,
matches the package version, resolves to the workflow commit, and that commit is
reachable from the protected default branch.

#### Scenario: Reject a tag outside protected main

- **WHEN** a version tag resolves to a commit that is not reachable from
  protected `main`
- **THEN** release eligibility fails before any platform build or credentialed
  publication job starts

#### Scenario: Reject a mismatched package version

- **WHEN** the tag's SemVer value differs from the Cargo package version
- **THEN** the release fails with an explicit version-binding error

### Requirement: Exact full CI authorizes publication

The release workflow SHALL require a completed successful `push` run of the
canonical CI workflow on protected `main` for the exact tagged commit. That run
SHALL contain exactly one completed successful `Required Quality Aggregate`
job, and the aggregate SHALL include the release policy and inventory contract
suite.

#### Scenario: Pull-request CI is not release authorization

- **WHEN** the tagged commit has a successful pull-request run but no
  successful `main` push run
- **THEN** release eligibility fails

#### Scenario: Aggregate failure blocks publication

- **WHEN** the exact CI run is missing, failed, or has no successful required
  aggregate job
- **THEN** no build, signing, release, or crate publication job runs

### Requirement: Release refs and credentials are protected

Version tags SHALL be immutable after creation, and jobs with repository write,
OIDC, attestation, or registry credentials SHALL use a protected publication
environment restricted to version tags.

#### Scenario: Attempt to move a version tag

- **WHEN** an actor attempts to update or delete a ref matching
  `refs/tags/v*`
- **THEN** the active tag ruleset rejects the operation without administrator bypass

#### Scenario: Publication waits for environment review

- **WHEN** policy, builds, and release quality succeed for an eligible tag
- **THEN** credentialed jobs remain pending until the `release` environment
  approves the matching `v*` tag deployment

## Implementation Plan

| Phase | Scope | Target | Exit evidence |
| --- | --- | --- | --- |
| 1 | Policy tool and fixtures | 2026-09-01 | Offline fail-closed tests |
| 2 | Workflow dependency and evidence | 2026-09-01 | YAML validation and focused release tests |
| 3 | Ruleset and environment | 2026-09-01 | GitHub API responses and settings links |
| 4 | Closeout | After PR CI | Green PR and merged-main CI evidence |
| 5 | Operational proof | Next version | G-02 immutable tag and clean-environment verification |

## Success Criteria

- Every invalid tag/source/CI fixture fails before publication.
- GitHub reports the tag ruleset and environment protections as active.
- Pull-request and merged-main CI pass without changing normal branch behavior.
- The next release retains policy, inventory, signature, and provenance evidence.

## Technical Example

```bash
python tools/release/release_policy.py verify \
  --tag "$GITHUB_REF_NAME" \
  --commit "$GITHUB_SHA" \
  --repository "$GITHUB_REPOSITORY" \
  --manifest tools/harness-gate/Cargo.toml \
  --output target/release-policy.json
```

## Alternatives Considered

- Reusing any successful CI run for the commit was rejected because event and
  branch identity are part of the authorization decision.
- Manual reviewer reconstruction of Git and CI state was rejected because the
  inputs are deterministic and should fail closed automatically.
- Making the release workflow a complete duplicate of CI was rejected because
  drift would remain possible and the exact protected-main run already exists.

## Rollback Plan

Revert the policy code through protected `main`, retain tag immutability, and
keep publication credentials behind the environment. If governance settings
must be changed, record the administrator action and restore the controls
before creating another version tag. Never move a published tag.
