# Design: Release Governance Hardening

## Eligibility Job

The tag workflow begins with a read-only policy job. A small standard-library
Python tool parses strict SemVer and Cargo metadata, asks Git to bind the tag,
commit, and fetched `origin/main`, and queries the GitHub Actions API for the
exact commit's successful `main` push run. It then requires one successful
`Required Quality Aggregate` job and atomically publishes JSON evidence.

Build and release-quality jobs depend on policy. The signed publication job
depends on policy, every platform build, and release quality. The crates.io job
depends on signed publication. A per-tag concurrency group prevents concurrent
publication attempts without cancelling an in-progress release.

## GitHub Governance

An active tag ruleset matches `refs/tags/v*` and contains update and deletion
rules with no bypass actor. It intentionally does not create a second naming
scheme: strict SemVer and package version are enforced by workflow policy.

The `release` environment uses a custom deployment policy of type `tag` with
pattern `v*`, an explicit maintainer reviewer, and no administrator bypass.
Both jobs that receive publication credentials reference the environment.

## Failure Model

Every missing or ambiguous input is a policy failure: malformed tags, missing
refs, non-main ancestry, GitHub API errors, no exact successful push run,
duplicate/missing aggregate jobs, and failed aggregate conclusions. The
workflow does not fall back to a branch-name guess, pull-request CI, or a
different successful commit.

## Compatibility

Normal branch and pull-request CI is unchanged. Existing release inventory and
signature behavior is unchanged after policy and environment approval. Legacy
`v1.0.0` is covered by immutability, but cannot trigger a new eligible release
because the current package version differs and tag creation is not replayed.

## Rollback

Revert the policy workflow and tool by pull request if eligibility logic is
incorrect. Retain immutable tags and the protected environment during code
rollback. Governance changes require an explicit administrator action and must
be recorded; moving an existing tag is never a rollback path.
