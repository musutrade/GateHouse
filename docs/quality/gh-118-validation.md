# GH-118: Generic shadow CI and Rust equivalence validation

Scope: OpenSpec `language-agnostic-evidence-policy-architecture` tasks 10.1–10.4.
Dependency GH-117 is closed; PR #126 merged at
`0eca68ea1d2d8f8998fbca715a51a18cc72e6367`, this branch's starting commit.

The [acceptance contract](rust-equivalence-acceptance.md) pins two retained Rust
runs at two distinct head commits. Both replayed successfully with identical
current/generic outcomes and no compatibility failures. All 15 adapter fixtures
passed, including malformed/missing evidence, measurement-error stages, outcome
and debt disagreements, unsupported-state drift and omitted policy results.
The runner retains hashes of the actual evaluator files and original artifacts.
This completes the defined retrospective window; it does not claim hosted shadow
runs or authorize replacing Rust release gates.

Before implementation, all 13 existing adapter tests passed. The new fixtures
expose and prevent an optional unsupported measurement becoming success or being
omitted while the aggregate stays green. Three additional acceptance/workflow
tests enforce the window and freeze the current aggregate name, all 15 existing
dependencies, event-specific requirements and fail-closed behavior. The advisory
job reuses the existing collection artifact and original event identities.
The `quality-required` job and `ci_quality.py` are unchanged.

Actual commands and outcomes are recorded in the
[machine summary](https://github.com/musutrade/Harness-Gate/blob/d9a932b23dcfea709175285b594ca200450a9971/docs/quality/gh-118/validation-summary.json). Complete logs, normalized
projections, policies and compatibility reports are retained in the
[evidence archive](https://github.com/musutrade/Harness-Gate/blob/d9a932b23dcfea709175285b594ca200450a9971/docs/quality/gh-118/local-evidence.tar.gz). Original candidates remain in
the linked GH-96/GH-97 archives; the archive avoids duplicating those raw files.

| Command | Actual result |
| --- | --- |
| `python3 tools/quality/rust_equivalence.py --output target/quality/gh-118/acceptance` | Passed: 2 distinct retained runs, 15 fixtures, no mismatches or skips |
| `cargo nextest run --manifest-path tools/harness-gate/Cargo.toml --locked` | Passed: 315 tests, 0 skipped |
| `cargo fmt --manifest-path tools/harness-gate/Cargo.toml -- --check` | Passed |
| `cargo clippy --manifest-path tools/harness-gate/Cargo.toml --all-targets -- -D warnings` | Passed |
| `python3 -m unittest discover -s tools/quality/tests -v` | Passed: 207 tests |
| `python3 tools/quality/docs_consistency.py --output target/quality/docs-consistency.json` | Passed |
| `openspec validate language-agnostic-evidence-policy-architecture --strict` | Passed |
| `harness-gate config check` | Not applicable: `.harness-gate/flow.toml` is absent |
| `harness-gate verify --profile ci --all` | Not applicable: no project-local `ci` profile |

Cargo build commands used `CARGO_TARGET_DIR=$PWD/target/gh-118-cargo`. No project
configuration was invented. Required hosted CI is pending and must pass before
merge; the controller owns CI observation, merge and issue closure.

[ADR-0040](../adr/0040-language-agnostic-evidence-policy.md) records Rust as the
reference adapter, noninterchangeable series, measurement/policy separation and
the fail-closed migration trust boundary. Legacy coverage and critical-path
stages retain their native authoritative results. The current Rust required
path remains the sole release authority. Task 10.5 and full proposal acceptance
remain pending; another ecosystem is outside this issue's scope.

The initial docs check failed before evidence links existed and because Cargo
used a read-only default build directory. Diagnostic command
`cargo run --manifest-path tools/harness-gate/Cargo.toml --locked -- config schema`
exited 101: `failed to open: /home/gem/cargo-target/debug/.cargo-build-lock`,
`Read-only file system (os error 30)`. Retaining evidence and setting the same
workspace-local `CARGO_TARGET_DIR` resolved the check. Both attempts are retained.

Delivery environment: `git add .github/workflows/ci.yml tools/quality/rust_reference.py`
exited 128: `fatal: Unable to create '/home/gem/symphony-workspaces/GH-118/.git/index.lock': Read-only file system`.
A copy of this workspace's Git metadata at `target/gh-118-git` is used with
`--git-dir=target/gh-118-git --work-tree=.` for commit/push on the same branch.
No other checkout is accessed.
