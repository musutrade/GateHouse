# GH-170: documentation and closure evidence

Scope: `optimize-ci-execution-topology` tasks 7.1–7.4 only, following merged
GH-169. The [operating guide](README.md) describes the final workflow and
diagnostics. The design now explicitly distinguishes rolled-back experiments
from retained behavior. Timing wording now explicitly starts at workflow run
creation, matching the normalizer rather than the PR's opening timestamp.
Engineering Policy and ADR-0039/0040 remain unchanged;
conditional cross-platform execution requires a separate normative policy delta
and was not implemented. No product workflow integration is included.

## Required-CI evidence after rollback

The final GH-169 [PR #176](https://github.com/musutrade/Harness-Gate/pull/176)
head `855b310caa58d6361a984609a36e531c95a0bbf8` was merged as
`7777511ec62c609aa664789a05d6dc376df30aa7`, this issue's prepared baseline.
Its [CI run 34315031483, attempt 1](https://github.com/musutrade/Harness-Gate/actions/runs/34315031483)
completed successfully. [Retained API evidence](https://github.com/musutrade/Harness-Gate/blob/d9a932b23dcfea709175285b594ca200450a9971/docs/quality/ci-topology/closure-hosted.json) binds PR,
head/merge identities, run/attempt and all 18 jobs with steps and conclusions.
This was a read-only capture of the completed dependency run; no CI was dispatched.

All twelve PR-required children succeeded: Linux Test, full Windows and macOS
Test, Build, Format, Clippy, Security Audit, Quality Coverage and Critical Paths,
Quality CLI Contracts, Documentation Consistency, Release Governance Contracts
and Quality Script Tests. `Required Quality Aggregate` succeeded; advisory
Generic Quality Shadow also succeeded. Native builds/contracts, Code Coverage
and Quality Performance Baseline were skipped under the existing PR exemption.
This is PR assurance evidence, not execution evidence for those push-only jobs.

The [task 6 comparison and parity review](after-state.md),
[comparison data](https://github.com/musutrade/Harness-Gate/blob/d9a932b23dcfea709175285b594ca200450a9971/docs/quality/ci-topology/hosted-comparison.json) and
[source boundary review](https://github.com/musutrade/Harness-Gate/blob/d9a932b23dcfea709175285b594ca200450a9971/docs/quality/ci-topology/semantic-parity-source.json) retain the before/after
cohort and assurance analysis. Run 34315031483 validates the code **after** the
individual rollbacks; it is not inserted into the earlier performance cohort
or used to extrapolate a new speedup.

## Final retained and rollback decisions

| Decision | Evidence and effect |
| --- | --- |
| Retain pinned, checksummed tool acquisition and version checks | Hosted install work falls materially; required installation failures still block. |
| Disable compiled target caches for all callers | Repeated zero-hit cohorts cost restore/save time. Preserve explicit targets, source caches and identity diagnostics. |
| Revert docs build-once execution | No meaningful hosted execution benefit. Preserve all eleven locked Cargo operations and semantic/failure checks. |
| Retain immutable quality manifest and independent digest validation | Clarifies producer/consumer identity without recollection or release authority. |
| Retain independent jobs and native tests | No hosted equivalence/cost evidence supports merging jobs or sharing incompatible binaries. |

Rollback remains execution-only: restore a prior setup/cache implementation
individually, preserve all required outcomes and measurement identities, retain
failure evidence, and rerun required CI. Do not loosen keys, lower thresholds,
remove native platforms or bypass artifact verification to retain an optimization.

## Validation and final acceptance boundary

[Exact local commands, results and log hashes](https://github.com/musutrade/Harness-Gate/blob/d9a932b23dcfea709175285b594ca200450a9971/docs/quality/ci-topology/closure-validation.json) record
the required nextest, format, Clippy, Python tests, docs consistency and strict
OpenSpec checks. Complete local logs are in `target/quality/gh170/`; the committed
record retains outcomes and hashes beyond ephemeral workspace logs.
All checks passed: 332 Rust tests, 328 Python tests, formatting, Clippy,
documentation consistency and strict OpenSpec validation.
Cargo uses this workspace's `target/gh170-cargo` for build output to respect the
workspace boundary. No project-local `.harness-gate/flow.toml` exists, so
`harness-gate config check` and `harness-gate verify --profile ci --all` are
**not applicable**, not passed; no synthetic profile was created.

GH-170's final submitted SHA still needs its own hosted Required Quality
Aggregate acceptance. The controller owns that result, merge, issue closure and
final OpenSpec acceptance. Task 7.4 remains unchecked until then; this record
does not declare the whole proposal implemented or archive it prematurely.
The final push is paired with an uncommitted `.symphony-handoff.json` containing
the actual PR, branch, full pushed SHA and validation summary. This declaration
and the controller's check results bind submission to acceptance; historical
green runs and Linux-local validation cannot certify the new SHA's hosted jobs.

The workspace's `.git` mount is read-only: local staging and committing fail
with an `index.lock` creation error, retained in the validation record. Delivery
uses the GitHub Git Data API to publish the exact workspace files on the prepared
branch, parented to its baseline. Local HEAD remains unchanged; the handoff
identifies the published commit for controller acceptance.
