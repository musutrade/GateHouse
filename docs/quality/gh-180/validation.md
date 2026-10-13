# GH-180 local validation

Date: 2026-09-09. Scope: OpenSpec tasks 2.1–2.3, ADR-0042.
These results describe the GH-180 changes on `symphony/GH-180`,
based on `caeeca3a34fb450e5de742fb8e09ff550d81a6bd`.

## Reproduction and compiler acceptance

Before implementation, the existing quality configuration suite passed 11 tests
(334 skipped); log: `target/quality/gh-180/before.log`.
The new retained compiler corpus is invoked by the shipped Rust CLI integration
suite. [Acceptance results](https://github.com/musutrade/Harness-Gate/blob/d9a932b23dcfea709175285b594ca200450a9971/docs/quality/gh-180/acceptance.json) contain 50 cases, with zero
unexplained semantic mismatches. Twenty cases compare direct and compiled
Rust reports byte-for-byte and their exit status; 25 negative cases reject
stale/mixed/malformed inputs; four reject output aliases without damaging inputs;
one proves context changes affect compilation identity. Every equivalence case
also verifies deterministic repeat compilation. Fixtures include an arbitrary
ecosystem, custom collector and series, with both supported and unsupported
metrics. No Python implementation decides these results.

## Required checks

All Cargo-dependent checks use `CARGO_TARGET_DIR="$PWD/target"`; the final
nextest rerun also uses `TMPDIR=/tmp` for the disk-space mitigation below.

| Command | Actual result | Log/artifact under `target/quality/` |
| --- | --- | --- |
| `cargo nextest run --manifest-path tools/harness-gate/Cargo.toml --locked` | 346 passed, 0 skipped, 161.416 seconds on final code | `gh-180/nextest-retry.log` |
| `cargo fmt --manifest-path tools/harness-gate/Cargo.toml -- --check` | Exit 0 on final code | `gh-180/fmt.log` |
| `cargo clippy --manifest-path tools/harness-gate/Cargo.toml --all-targets -- -D warnings` | Exit 0 on final code | `gh-180/clippy-final.log` |
| `python3 -m unittest discover -s tools/quality/tests -v` | 329 tests, OK, 231.949 seconds | `gh-180/python.log` |
| `python3 tools/quality/docs_consistency.py --output target/quality/docs-consistency.json` | Exit 0, status pass | `docs-consistency.json`, `gh-180/docs.log` |
| `openspec validate integrate-generic-quality-into-project-workflow --strict` | Exit 0, valid | `gh-180/openspec.log` |
| `python3 tools/quality/fixtures/workflow/compiler/acceptance.py --harness-gate target/debug/harness-gate --output target/quality/gh-180/corpus-final-50` | Exit 0, 50 cases, zero unexplained mismatches on final binary | `gh-180/corpus-final.log`, `gh-180/corpus-final-50/acceptance.json` |

The Python source-measurement contract test includes the new compiler module;
the production inventory and versioned measured source selection include it.
The AST analyzer reports maximum function complexity 24 in the changed CLI
module and 16 in the new compiler. This is a local structural check, not a
replacement for source-verified base/head CRAP evaluation.

## Additional production coverage

`TMPDIR=/tmp CARGO_TARGET_DIR="$PWD/target" python3 tools/quality/coverage.py --production --output "$PWD/target/quality/gh-180/production-final.json"`
exited 0 on final code. Every blocking boundary passes the unchanged 80%
threshold. Config line coverage is 2939/3256 (90.26%); aggregate line coverage
is 12385/14048 (88.16%). [Boundary results](production-coverage.md) are retained.
Full source hashes, inventory, regions and raw export references are in
`target/quality/gh-180/production-final.json`; execution is logged in
`target/quality/gh-180/production-final.log`. This local coverage result does
not establish a source-verified base/head risk comparison or hosted CI success.

## Environment and delivery limitations

Cargo-dependent commands use `CARGO_TARGET_DIR="$PWD/target"` because the
inherited `/home/gem/cargo-target` is read-only. Logs and complete generated
corpus reports are retained under `target/quality/gh-180/` in this workspace.

`harness-gate config check` and `harness-gate verify --profile ci --all` are
**not applicable**: this source checkout has no `.harness-gate/flow.toml`.
No project-local configuration was invented for these checks.

Git staging and commit were attempted, but Git could not create its lock:

```text
fatal: Unable to create '/home/gem/symphony-workspaces/GH-180/.git/index.lock': Read-only file system
```

The initial staging command, `git add docs/quality-compilation.md`, exited
128 with that error (`target/quality/gh-180/staging.log`). On retry, the existing
Git metadata was copied into `target/quality/gh-180/delivery.git` inside this
same workspace. Delivery commands use that writable metadata with the current
workspace as their work tree, preserving `symphony/GH-180` and its baseline.
The original read-only `.git` is unchanged; no other workspace or source
checkout was accessed. The retry verified the retained full-suite logs and
`git diff --check`; unchanged code was not unnecessarily remeasured.

The source-verified base/head risk comparison has not been run locally.
Hosted Required Quality Aggregate and controller acceptance remain pending;
local checks are not a claim of hosted acceptance.

## CI repair attempt 1

PR #189 at `34f4ccbf5e45953cee8662825780210f0dd3c301` failed
`Test (windows-latest)`: 304 tests passed and the compiler acceptance test failed
with `('pass', 'ERROR [E1000]: command failed: mixed configuration file inventory')`.
The [failed Windows job](https://github.com/musutrade/Harness-Gate/actions/runs/34331423091/job/102400652657)
blocked Required Quality Aggregate. Full failed-job output is retained locally in
`target/quality/gh-180/windows-ci-failure.log`.

The fixture used `str(Path.relative_to(root))` as a contract key. Reproduction
with `PureWindowsPath` confirms that this emits `.harness-gate\\flow.toml`,
which does not match the configured `.harness-gate/flow.toml`. The fixture now
uses `as_posix()` to preserve the configured spelling across platforms. This
changes fixture serialization only; the compiler still rejects mixed inventory
and stale digests, and Rust remains the sole decision authority. The existing
native CLI acceptance test exercises this path on every supported CI platform.

Repair evidence is under `target/quality/gh-180/repair-*`. The 50-case corpus,
formatting, Clippy, docs consistency, and strict OpenSpec validation pass.
The corpus includes the unknown ecosystem and all existing negative cases.
The required nextest command passed 346 tests, 0 skipped, in 177.507 seconds
(`repair-nextest.log`), with `TMPDIR=/tmp CARGO_TARGET_DIR="$PWD/target"`.
The required Python unittest command passed 329 tests in 198.784 seconds
(`repair-python.log`) with the same environment. Exact commands are listed in
the required-checks table above; repair logs for format, Clippy, OpenSpec and
the corpus are `repair-fmt.log`, `repair-clippy.log`, `repair-openspec.log`, and
`repair-corpus.log`. The repaired corpus report is `repair-corpus/acceptance.json`.

The initial docs command without the workspace Cargo target override failed
with `quality gate failed: documentation, examples, or schema synchronization failed`.
Its Cargo subprocess failure was confirmed with
`cargo run --quiet --locked --manifest-path tools/harness-gate/Cargo.toml -- schema export`:
`error: failed to open: /home/gem/cargo-target/debug/.cargo-build-lock`,
`Read-only file system (os error 30)`. Repeating docs consistency with
`TMPDIR=/tmp CARGO_TARGET_DIR="$PWD/target"` passed. The failure and retry logs are
`repair-docs.log`, `repair-default-target-error.log`, and `repair-docs-retry.log`.
The project-local config/verify commands remain not applicable. Native Windows
execution and Required Quality Aggregate after this repair remain CI pending.
