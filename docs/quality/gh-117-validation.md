# GH-117: Cross-component contracts and project reporting validation

Scope: OpenSpec `language-agnostic-evidence-policy-architecture` tasks 8.1–8.3
and 9.1–9.3. Dependency GH-116 is closed; its PR #125 is merged at
`b21ae2d86c1d89058eb7e16f15eeaac7e90bcc73`, this checkout's starting commit.

The [configuration and reporting decision](project-reporting.md) documents the
additive JSON manifest, explicit contract bindings, provenance checks, generic
metrics and lossless report indexes. Existing Rust `flow.toml` behavior and
defaults remain unchanged. The single Rust fixture exits 0. The synthetic
Angular + Rust + Python + Java fixture exits 1: all four local gates pass, but
the response-field rename fails breaking-change, generated-client drift and
consumer compatibility gates. Both participants inherit those failures; the
project counts each shared gate once.

The [12 acceptance tests](../../tools/quality/tests/test_project_report.py)
also cover a coherent compatible contract, multiple consumers with separate
series, generic policy comparisons, unavailable metrics, missing evidence,
tampered artifacts, stale or incompatible baselines, wrong participants,
missing consumer/client evidence and contradictory drift facts. Manifest tests
cover explicit opt-in, legacy configuration preservation, unknown versions and
keys, path traversal and symlink escape. Existing quality tests still pass.
Before implementation, the focused policy suite passed 9 tests.

Actual commands and outcomes are recorded in the
[machine summary](https://github.com/musutrade/Harness-Gate/blob/d9a932b23dcfea709175285b594ca200450a9971/docs/quality/gh-117/validation-summary.json). Complete logs and machine
reports are retained in the [evidence archive](https://github.com/musutrade/Harness-Gate/blob/d9a932b23dcfea709175285b594ca200450a9971/docs/quality/gh-117/local-evidence.tar.gz).

| Command | Actual result |
| --- | --- |
| `cargo nextest run --manifest-path tools/harness-gate/Cargo.toml --locked` | Passed: 315 tests, 0 skipped |
| `cargo fmt --manifest-path tools/harness-gate/Cargo.toml -- --check` | Passed |
| `cargo clippy --manifest-path tools/harness-gate/Cargo.toml --all-targets -- -D warnings` | Passed |
| `python3 -m unittest discover -s tools/quality/tests -v` | Passed: 202 tests |
| `python3 tools/quality/docs_consistency.py --output target/quality/docs-consistency.json` | Passed |
| `openspec validate language-agnostic-evidence-policy-architecture --strict` | Passed |
| `harness-gate config check` | Not applicable: `.harness-gate/flow.toml` is absent |
| `harness-gate verify --profile ci --all` | Not applicable: no project-local `ci` profile |

Cargo build commands used `CARGO_TARGET_DIR=$PWD/target/gh-117-cargo`. The final
Python suite used `TMPDIR=$PWD/target/gh-117-tmp`. No project configuration was
invented. Required hosted CI is pending and must pass before merge.

This is shadow-only architecture validation. Contract measurements and non-Rust
ecosystems are synthetic; no OpenAPI parser or client generator is implemented
or certified. Baseline provenance validates caller-retained artifacts and pinned
identity, without independently resolving Git objects. The
[Rust reference adapter](rust-reference-adapter.md) and
[ADR-0039](../adr/0039-required-risk-and-traceability-gates.md) remain authoritative.
Tasks 10.1–10.5 and final architecture acceptance remain pending.

Delivery environment: `git add tools/quality/cross_component.py` returned exit 128:
`fatal: Unable to create '/home/gem/symphony-workspaces/GH-117/.git/index.lock': Read-only file system`.
A copy of this workspace's Git metadata at `target/gh-117-git` is used with
`--git-dir=target/gh-117-git --work-tree=.` for commit/push on the same branch.
No other checkout is accessed.
