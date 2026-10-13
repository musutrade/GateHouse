# GH-135: advisory frontend CI and bounded certification

Scope: [OpenSpec task 4.2](../../../openspec/changes/typescript-angular-reference-adapter/tasks.md),
following [ADR-0040](../../adr/0040-language-agnostic-evidence-policy.md).
GH-134 is closed and its implementation is present in baseline commit
`2de3b1c0baf909f7b5f6a0f52fb1b893b6f25fbc` (merged PR #142).

The manual [Frontend Advisory workflow](../../../.github/workflows/frontend-advisory.yml)
runs the [advisory runner](https://github.com/musutrade/Harness-Gate/blob/8d93ea53c7cf8b314bda816eafe13209da25c742/tools/quality/typescript_advisory.py) using
retained GH-134 coverage and GH-133 contract artifacts. It performs no native
frontend collection. Its 30-day artifact upload runs even after failure and
includes raw replay inputs, normalized evidence, policy/project reports, test
logs, machine-readable summary and SHA-256 inventory. Committed evidence and
baselines remain durable. The runner rejects reused output directories and
retains failure summaries; two new publication/integrity tests cover those cases.

The [certification matrix](../typescript-certification.md) names exact fixture
revisions, tools, native/replay environments, coverage and contract series,
supported capabilities and explicit unavailable states. TS-01–TS-04 each link
reproducers and evidence. Rollback disables the manual workflow or opt-in adapter
selection while retaining evidence and accepted baselines.

[Workflow validation](https://github.com/musutrade/Harness-Gate/blob/d9a932b23dcfea709175285b594ca200450a9971/docs/quality/gh-135/workflow-validation.json) parses the workflow and confirms
manual-only dispatch, read-only permissions, independent job, always-upload and
30-day retention. SHA-256 comparisons against the baseline confirm `ci.yml`,
`release.yml`, `ci_quality.py` and the fixture toolchain are byte-identical.
Required Quality Aggregate, required dependencies, check identity, Rust defaults
and release authority are unchanged.

## Actual validation

Complete local logs and summaries are retained in `validation.tar.gz`, indexed by
[artifact digests](https://github.com/musutrade/Harness-Gate/blob/d9a932b23dcfea709175285b594ca200450a9971/docs/quality/gh-135/index.json). [Advisory summary](https://github.com/musutrade/Harness-Gate/blob/d9a932b23dcfea709175285b594ca200450a9971/docs/quality/gh-135/summary.json) records the native
fixture revisions and full measurement series. Its checkout field is the
pre-commit baseline used for this local working-tree run; it is not the PR SHA.
Hosted executions record the actual checked-out SHA and environment.

| Command | Result |
| --- | --- |
| `python3 tools/quality/typescript_acceptance.py --work target/quality/gh135/before` | Exit 0; pre-change confirmation of retained coverage flow |
| `python3 tools/quality/typescript_advisory.py --output target/quality/gh135/advisory-v2` | Exit 0; 88 exact counter comparisons; compatible pass and deliberate regression fail for threshold/debt; compatible contract pass and breaking contract fail with green local gates; 22 acceptance and 11 contract tests passed |
| `cargo nextest run --manifest-path tools/harness-gate/Cargo.toml --locked` | Exit 0; 315 passed, 0 skipped |
| `cargo fmt --manifest-path tools/harness-gate/Cargo.toml -- --check` | Exit 0 |
| `cargo clippy --manifest-path tools/harness-gate/Cargo.toml --all-targets -- -D warnings` | Exit 0 |
| `python3 -m unittest discover -s tools/quality/tests -v` | Exit 0; 285 passed |
| `python3 tools/quality/docs_consistency.py --output target/quality/docs-consistency.json` | Exit 0; status `pass` |

Cargo commands and Cargo subprocesses use `CARGO_TARGET_DIR=$PWD/target` within
this workspace. Local Python is 3.14.4; the hosted replay specifies Python 3.12
on ubuntu-24.04 and has not yet run. YAML structure was checked with PyYAML;
actionlint is not installed, so no actionlint pass is claimed.

`harness-gate config check` and `harness-gate verify --profile ci --all` are **not
applicable**: this checkout has no `.harness-gate/flow.toml`. No configuration was
invented. There are no missing dependencies blocking the declared local checks.

The initial advisory command with `--output target/quality/gh135/advisory` exited
1: `CollectionError: workspace_root must be an existing canonical absolute directory`.
The runner now resolves the output path before invoking the accepted collector;
the fresh `advisory-v2` run passes. The initial failure summary remains in the
validation archive. The native collection artifacts were not changed.

The first documentation check exited 1 because this evidence record linked the
not-yet-written `index.json`. The index was created before repeating the check.

`git add .github/workflows/frontend-advisory.yml ...` exited 128:
`fatal: Unable to create '/home/gem/symphony-workspaces/GH-135/.git/index.lock': Read-only file system`.
Only this workspace's Git metadata was copied to ignored
`target/quality/gh135/delivery.git`, after verifying there were no symlinks,
shared common directory or external object alternates. This copy commits and
pushes the same branch/working tree; no other workspace is accessed.

Required hosted CI on the final pushed SHA is **pending**. Task 4.2 remains
unchecked until it passes before merge; task 4.3 review and proposal closure are
outside this issue. The host controller owns CI observation and merge.
