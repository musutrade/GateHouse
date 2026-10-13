# GH-164 validation

Scope: `optimize-ci-execution-topology` tasks 1.1–1.3. See the
[frozen contract and hosted evidence](baseline.md).

Before editing, `python3 -m unittest discover -s tools/quality/tests -p test_ci_quality.py -v`
passed all 11 existing tests. Reproducing the original `COMMON`/`PUSH_ONLY`
classification with a PR `test-cross-platform: failure` returned `[]`, confirming
the enforcement gap. The independent frozen fixture now tests every required
child on both events with missing child, missing result, failure, cancellation
and skip. A CLI regression proves native PR failure returns exit 1. Workflow
contract tests retain exact job/check names, dependencies, OS matrices, native
full-test commands and aggregate environment/command behavior.

The hosted baseline reproduces from retained API timestamps and validates
required native instances, attempts, OS attribution and negative evidence.
All three pre-change hosted `Required Quality Aggregate` jobs succeeded.
The submitted commit's hosted aggregate remains subject to controller CI;
historical success is not a claim that this PR has passed.

## Local validation

| Command | Result |
| --- | --- |
| `CARGO_TARGET_DIR="$PWD/target/gh-164-build" cargo nextest run --manifest-path tools/harness-gate/Cargo.toml --locked` | Exit 0; 332 passed, 0 skipped. |
| `cargo fmt --manifest-path tools/harness-gate/Cargo.toml -- --check` | Exit 0. |
| `CARGO_TARGET_DIR="$PWD/target/gh-164-build" cargo clippy --manifest-path tools/harness-gate/Cargo.toml --all-targets -- -D warnings` | Exit 0. |
| `python3 -m unittest discover -s tools/quality/tests -v` | Exit 0; 303 tests passed on final rerun. |
| `CARGO_TARGET_DIR="$PWD/target/gh-164-build" python3 tools/quality/docs_consistency.py --output target/quality/docs-consistency.json` | Exit 0; report status pass. |
| `python3 -m unittest discover -s tools/quality/tests -p 'test_ci_t*.py' -v` | Exit 0; all 6 topology/timing tests passed. |
| `python3 -m py_compile tools/quality/*.py tools/quality/tests/*.py` | Exit 0. |
| `openspec validate optimize-ci-execution-topology --strict` | Exit 0. |
| `git diff --check` | Exit 0. |

The initial full Python suite ran 303 tests with one inventory-completeness
failure: `tools/quality/ci_timing.py` was missing from the production Python
module inventory. The GH-146 and GH-152 inventories now classify this manual
measurement utility as maintained category A tooling with no decision authority.

The inherited `CARGO_TARGET_DIR=/home/gem/cargo-target` is outside this workspace's
writable roots. The initial exact nextest and Clippy commands returned exit 101:
`error: failed to open: /home/gem/cargo-target/debug/.cargo-build-lock`, caused by
`Read-only file system (os error 30)`. Initial docs consistency returned exit 1
because its Cargo-dependent examples, migration and schema checks could not run.
Reruns set `CARGO_TARGET_DIR="$PWD/target/gh-164-build"`; no project configuration
or source checkout is changed. Logs are retained under `target/quality/gh-164/`.

`harness-gate config check` and `harness-gate verify --profile ci --all` are
**not applicable**: this checkout has no `.harness-gate/flow.toml` declaring a
`ci` profile. No synthetic configuration was created.

## Submission environment

The workspace also mounts `.git` read-only. The staging command `git add` returned
exit 128: `fatal: Unable to create '/home/gem/symphony-workspaces/GH-164/.git/index.lock': Read-only file system`.
Submission therefore uses the GitHub Git database API to create a commit from
these exact workspace files, parented by the controller-prepared branch HEAD,
and publish `symphony/GH-164`. Local Git metadata cannot be updated in this
session; the handoff records the actual published remote commit. No source
checkout or other workspace is accessed or modified.
