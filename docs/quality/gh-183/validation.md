# GH-183 validation

Scope: OpenSpec `integrate-generic-quality-into-project-workflow`, tasks 5.1–5.5.
See [ADR-0045](../../adr/0045-quality-verification-composition.md) and the
[workflow contract](../../quality-verification.md).

Full command logs are retained under `target/quality/gh-183/`. Cargo-dependent
commands use `CARGO_TARGET_DIR=$PWD/target` because the environment's default
target is outside the writable workspace. No thresholds or required CI jobs
are changed.

| Command | Actual result | Log |
| --- | --- | --- |
| `cargo nextest run --manifest-path tools/harness-gate/Cargo.toml --locked` | 360 passed, 0 skipped | `nextest-final.log` |
| `cargo fmt --manifest-path tools/harness-gate/Cargo.toml -- --check` | Passed | `fmt-final.log` |
| `cargo clippy --manifest-path tools/harness-gate/Cargo.toml --all-targets -- -D warnings` | Passed | `clippy-final.log` |
| `python3 -m unittest discover -s tools/quality/tests -v` | 330 passed | `python-tests-final.log` |
| `python3 -m unittest discover -s tools/quality/tests -p test_verify_quality_architecture.py -v` | 2 passed after adding the arbitrary-identifier switch guard control | `architecture-final.log` |
| `python3 tools/quality/docs_consistency.py --output target/quality/docs-consistency.json` | Passed | `docs-consistency-final.log` |
| `openspec validate integrate-generic-quality-into-project-workflow --strict` | Passed | `openspec-final.log` |

The Rust tests exercise a signed collector with ecosystem
`nebula-unregistered-2049`, verify scope selection (all, changed files and
components), immutable retained baseline resolution, compilation, the released
evaluator and both authoritative and unified reports. Entire direct CLI reports
and decisions match. The success/failure matrix covers independent execution and
quality outcomes, malformed evidence, configuration mutation, invalid trusted
state/selection/baseline, and an audit failure despite generic success. A CRAP
case retains base 35, head 40, threshold 30, ratchet, evidence and remediation in
the human report. The architecture guard checks generic verify and project-report
sources for closed ecosystem enums, language dispatch and ecosystem switches.

The initial Rust run caught a stale generated quality schema; regeneration and
the full final run passed. The initial Python run caught the changed declaration
file's source-inventory hash. AST inspection confirmed it still has no handwritten
production functions or closures; updating the hash and rerunning passed. The
format check caught one test line that was subsequently formatted. Initial logs
are retained alongside the final logs.

Production coverage passes: **13,219 / 14,887 lines (88.796%)**, with every
blocking boundary passing its unchanged threshold. The instrumented nextest run
also passed all 360 tests. The [coverage summary](https://github.com/musutrade/Harness-Gate/blob/d9a932b23dcfea709175285b594ca200450a9971/docs/quality/gh-183/coverage-summary.json) retains
aggregate and boundary results. Its source inventory includes the new verify
composition source; source-risk selection advances to
`gh183-quality-verification/1` for both base and head without threshold changes.
New composition functions have measured cyclomatic complexity at most 14
(`verify-quality-ast.json` and `complexity-summary.json` in the log directory).

Coverage commands use `CARGO_TARGET_DIR=$PWD/target/gh183-coverage`,
`CARGO_INCREMENTAL=0`, `CARGO_PROFILE_DEV_DEBUG=0` and
`CARGO_PROFILE_TEST_DEBUG=0` to keep artifacts within the writable workspace and
limit disk use:

```bash
cargo llvm-cov nextest --package harness-gate --manifest-path tools/harness-gate/Cargo.toml --locked --test-threads 2 --no-fail-fast --json --output-path target/quality/gh-183/coverage.raw.json
cargo llvm-cov report --package harness-gate --manifest-path tools/harness-gate/Cargo.toml --lcov --output-path target/quality/gh-183/coverage.lcov
cargo llvm-cov report --package harness-gate --manifest-path tools/harness-gate/Cargo.toml --cobertura --output-path target/quality/gh-183/coverage.cobertura.xml
python3 tools/quality/coverage.py --production --raw target/quality/gh-183/coverage.raw.json --lcov target/quality/gh-183/coverage.lcov --output target/quality/gh-183/production.json
```

All four commands passed. Logs: `coverage.log`, `coverage-export.log` and
`coverage-production.log`. Hosted source-risk comparison remains pending;
local coverage does not establish hosted Required Quality Aggregate status.

`harness-gate config check` and `harness-gate verify --profile ci --all` are
**not applicable**, not passed: this source checkout has no
`.harness-gate/flow.toml` declaring a `ci` profile. No synthetic root project
configuration was added. Hosted Required Quality Aggregate is **CI pending**;
the controller owns that check and acceptance.

Local staging is blocked: `git add tools/harness-gate/src/verify/quality.rs`
failed with `Unable to create '/home/gem/symphony-workspaces/GH-183/.git/index.lock':
Read-only file system`. Publication therefore uses GitHub's Git tree/commit/ref
API on `symphony/GH-183`, preserving the controller's prepared parent and exactly
the validated workspace files. Local Git metadata remains unchanged; the runtime
handoff identifies the actual remote commit. No other checkout is accessed.

## PR #192 CI repair

The retained run `34351946457` for `a727bd4fefd3cb752c607efce1ad8d84da3b4fea`
failed in macOS tests and the required quality collector. Repair command logs
and downloaded CI evidence reside under `target/quality/gh-183-repair/`.

The baseline reader now canonicalizes its repository root before checking input
containment. This handles macOS `/var` aliases to `/private/var`. A Unix test
accepts an aliased root and still rejects a symlink escaping the repository.
Linux exercises that condition; the hosted macOS rerun remains pending.

Risk selection now includes the omitted `failure.rs` and `verify/report.rs`,
with an AST/instrumentation certification regression. Selection series
`gh183-quality-verification/2` applies identically to base and head. Critical-path
function spans, source hashes and probe lines now match verify/report code.
Restoring the failure registry to measurement exposed two duplicated wire-name
mappings with complexity 32 and 33. Display and parsing now share the existing
Serde contract; a regression checks all 32 explicit wire names against the
schema, round-trips every name and rejects unknown names. Their measured
complexities are now 3 and 1 (`failure-complexity.json`). No quality thresholds,
required jobs, generic semantics or ecosystem dispatch changed.

Validation before the final failure-registry refactor passed the exact required
nextest command (361 tests, `nextest.log`), Python discovery (332 tests,
`python.log`), formatting (`fmt.log`), Clippy (`clippy.log`) and documentation
consistency (`docs.log`). The registry refactor then passed its focused nextest
regression (`failure-registry.log`) and all 361 tests in the complete coverage
suite (`candidate-v5/legacy.log`). Production coverage on that source passed all
blocking boundaries: 13,222 / 14,890 lines (88.798%), with raw JSON, LCOV and
Cobertura retained in `candidate-v5/`.

The final required nextest and Python retries were interrupted when the shared
filesystem exhausted space (`nextest-final.log`, `python-final.log`); those
incomplete retries are not passes. The concurrent command
`cargo clippy --manifest-path tools/harness-gate/Cargo.toml --all-targets -- -D warnings`
failed writing `target/debug/deps/rmetaTgFEhY/full.rmeta` with
`No space left on device (os error 28)` (`clippy-final.log`). Formatting passed
on the final source (`fmt-final.log`). After removing disposable artifacts,
Clippy passed (`clippy-verified.log`) and the exact documentation consistency
command passed (`docs-verified.log`, `target/quality/docs-consistency.json`).
Final Python discovery also passed all 332 tests (`python-verified.log`).
A final exact nextest retry with `TMPDIR=/tmp` and `NEXTEST_TEST_THREADS=2`
failed in two oracle tests with `shutil.Error` / `[Errno 122] Disk quota exceeded`
while copying fixtures into `/tmp/.tmpib2xKg/`; 359 tests were not run
(`nextest-verified.log`). This is distinct from the successful 361-test coverage
suite and the earlier successful required nextest run.

The exact risk command `cargo llvm-cov nextest --workspace --locked --manifest-path
$PWD/target/quality/gh-183-repair/candidate-v5/snapshots/base/tools/harness-gate/Cargo.toml
--json --output-path $PWD/target/quality/gh-183-repair/candidate-v5/base-coverage.json`
failed because baseline oracle tests could not write `cases.json`:
`OSError: [Errno 28] No space left on device`. A retry with
`NEXTEST_TEST_THREADS=2` hit the same limit (`base-coverage.log`,
`base-coverage-disk-full.log`, `resume-v5-final.log` beneath the repair artifacts).
Consequently the complete base/head risk comparison is not locally verified,
and no Required Quality Aggregate success is claimed.

The matrix collected all 12 exact tests and fresh profile exports, then failed
in lock cleanup because the interrupted earlier collector removed its lock.
The saved bundle was subsequently validated with the unchanged evaluator:

```bash
python3 tools/quality/critical_paths.py --evidence target/quality/gh-183-repair/candidate-v5/critical-path-runs/bundle.json --output target/quality/gh-183-repair/critical-paths-verified.json
```

Result: **12/12 applicable Linux paths passed (100%)**, with no failures
(`matrix-verified.log`, `critical-paths-verified.json`). The failed collection
status remains preserved; only this independent bundle validation is a pass.

Local collection also encountered leaked alternate Git variables in fixture
subprocesses, stale coverage paths from reused snapshot artifacts, and an
interrupted critical-path collection. Those attempts remain retained as failed
or incomplete evidence. The final collector invocation isolates fixture Git
variables with `bin/cargo`, uses fresh snapshot paths, and retains successful
raw production coverage unchanged. Local wrappers are artifacts, not changes to
the CI implementation. The initial documentation command also required replacing
the ambient read-only Cargo target with `CARGO_TARGET_DIR=$PWD/target`.

Publication uses a workspace-local bare Git metadata copy at
`target/quality/gh-183-repair/git-publish`, preserving PR #192's existing history.
The controller-managed `.git` stays unchanged; no other workspace is accessed.
Root `harness-gate config check` and `harness-gate verify --profile ci --all`
remain not applicable: this checkout has no `.harness-gate/flow.toml` declaring
`ci`. Hosted macOS and Required Quality Aggregate status remain CI pending.
