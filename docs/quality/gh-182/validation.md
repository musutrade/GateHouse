# GH-182 trusted baseline validation

Scope: OpenSpec `integrate-generic-quality-into-project-workflow`, tasks 4.1–4.4;
[ADR-0044](../../adr/0044-trusted-quality-baselines.md).

Runtime logs are retained under `target/quality/gh-182/`. Rust and embedded Rust
checks use `CARGO_TARGET_DIR=/tmp/gh182-cargo`, `CARGO_INCREMENTAL=0`,
`CARGO_PROFILE_DEV_DEBUG=0`, and `CARGO_PROFILE_TEST_DEBUG=0`.

| Command | Result | Evidence |
| --- | --- | --- |
| `cargo nextest run --manifest-path tools/harness-gate/Cargo.toml --locked` | 354 passed, 0 skipped | `nextest.log` |
| `cargo fmt --manifest-path tools/harness-gate/Cargo.toml -- --check` | Passed | `fmt.log` |
| `cargo clippy --manifest-path tools/harness-gate/Cargo.toml --all-targets -- -D warnings` | Passed | `clippy.log` |
| `python3 -m unittest discover -s tools/quality/tests -v` | 329 passed | `python.log` |
| `python3 tools/quality/docs_consistency.py --output target/quality/docs-consistency.json` | Passed | `docs.log`, output JSON |
| `openspec validate integrate-generic-quality-into-project-workflow --strict` | Passed | `openspec.log` |
| `python3 tools/quality/fixtures/workflow/baseline/acceptance.py --harness-gate /tmp/gh182-cargo/debug/harness-gate --output target/quality/gh-182/baseline-validated` | 39 cases passed | `baseline-validated.log`, `baseline-validated/acceptance.json` |

The committed [corpus summary](https://github.com/musutrade/Harness-Gate/blob/d9a932b23dcfea709175285b594ca200450a9971/docs/quality/gh-182/baseline-corpus.json) records each expected
resolution outcome. The integration test invokes the same fixture through the
shipped CLI. Cases cover exact Git/ref/merge-base and retained-artifact resolution,
unknown ecosystem with a custom series and supported numeric custom capability,
rename/move inherited debt, worsening debt, missing required/optional baselines,
stale run/commit, changed configuration/tool, incompatible series, corrupt hashes,
unsafe paths, and unchanged dirty working-tree/index bytes. Resolved inputs match
the direct evaluator's report and exit status; custom metric transport does not
extend the evaluator's certified metric registry.

Final production coverage passes: **12,943 / 14,611 lines (88.584%)**, with every
blocking production boundary passing its unchanged threshold. The final
instrumented nextest run also passed all 354 tests. Commands, with the workspace
coverage target described below:

```bash
cargo llvm-cov nextest --package harness-gate --manifest-path tools/harness-gate/Cargo.toml --locked --test-threads 2 --no-fail-fast --json --output-path target/quality/gh-182/coverage.raw.json
cargo llvm-cov report --package harness-gate --manifest-path tools/harness-gate/Cargo.toml --lcov --output-path target/quality/gh-182/coverage.lcov
cargo llvm-cov report --package harness-gate --manifest-path tools/harness-gate/Cargo.toml --cobertura --output-path target/quality/gh-182/coverage.cobertura.xml
python3 tools/quality/coverage.py --production --raw target/quality/gh-182/coverage.raw.json --lcov target/quality/gh-182/coverage.lcov --output target/quality/gh-182/production.json
```

Logs: `coverage-retry.log`, `coverage-export.log`, `coverage-production.log`.
The [coverage summary](https://github.com/musutrade/Harness-Gate/blob/d9a932b23dcfea709175285b594ca200450a9971/docs/quality/gh-182/coverage-summary.json) retains aggregate and boundary
results. Hosted source risk comparison and Required Quality Aggregate are pending;
local production coverage alone does not establish their status.

The default Cargo target `/home/gem/cargo-target` is read-only in this workspace:
`cargo check --manifest-path tools/harness-gate/Cargo.toml --locked` failed opening
`debug/.cargo-build-lock` with `Read-only file system (os error 30)`.
Using `CARGO_TARGET_DIR=/tmp/gh182-cargo` fixed that restriction. The first full
nextest build then failed before test execution with `Disk quota exceeded (os
error 122)`. Its log is `nextest-quota.log`. Cargo's own clean command removed
this task's build artifacts. Subsequent Rust builds also set `CARGO_INCREMENTAL=0`,
`CARGO_PROFILE_DEV_DEBUG=0` and `CARGO_PROFILE_TEST_DEBUG=0` to reduce disk use.

An initial docs check without the writable Cargo environment could not run its
embedded Cargo example/schema checks. It also found the then-unwritten validation
document. The final docs check uses the same writable Cargo environment.

`harness-gate config check` and `harness-gate verify --profile ci --all` are **not
applicable**, not passed: this source checkout has no `.harness-gate/flow.toml`
declaring a `ci` profile. No synthetic project configuration was added.

The first coverage target in `/tmp/gh182-coverage` also exhausted its disk quota
(`coverage.log`). Cargo cleaned only this task's target. The final instrumented
run uses `CARGO_TARGET_DIR=$PWD/target/gh182-coverage` with the same other Rust
environment settings. No coverage or risk threshold was changed.

An additional `cargo llvm-cov nextest --package harness-gate --manifest-path
tools/harness-gate/Cargo.toml --locked --json --output-path
target/quality/gh-182/coverage.raw.json` run hit the existing cancellation test's
15-second startup deadline (`verification_cleanup_cancel_and_report_precedence_retains_resource_evidence`:
`task did not start`). `coverage-final.log` retains that failure. The required
uninstrumented nextest run passed that test. Final coverage uses `--test-threads 2 --no-fail-fast`; the cancellation test passes at that concurrency.

Local staging is blocked: `git add` failed with
`Unable to create .../.git/index.lock: Read-only file system`. Publication uses
GitHub's Git tree/commit/ref API on `symphony/GH-182`, preserving the prepared
parent commit and publishing exactly the validated workspace files. Local Git
metadata remains unchanged; the handoff identifies the actual remote commit.

Hosted Required Quality Aggregate is **CI pending**, owned by the controller.
This change does not alter required jobs, measurement thresholds, historical
baseline acceptance, or mark the remaining OpenSpec tasks complete.

## PR #191 CI repair, attempt 1

The aggregate for `6f46112c2179c5578d8c4857b4f6bca696f9e827` failed because
`test-cross-platform` failed. The completed [CI run](https://github.com/musutrade/Harness-Gate/actions/runs/34345060772)
identified two test-fixture issues:

- Windows Git normalized generated CRLF configuration files at commit time.
  The baseline correctly rejected `.harness-gate/coverage-request.json` because
  its committed bytes differed from the trusted fixture hash. The fixture now
  sets repository-local `core.autocrlf=false` before staging its exact bytes.
- The macOS collector CLI test exceeded its 5,000 ms adapter timeout during
  interpreter startup. Normal fixture requests now allow 30 seconds, with a
  matching fixture host limit. The deliberate timeout case retains its 50 ms
  deadline; production timeout defaults and enforcement are unchanged.

The Windows failure was reproduced locally by generating CRLF files with Git
normalization enabled. All 39 baseline cases then passed with the fix, simulated
Windows text writes, and a global `core.autocrlf=true` setting. The existing
native-platform test continues to exercise this corpus. Native Windows/macOS
reruns and the new Required Quality Aggregate remain CI pending.

Repair logs are retained under `target/quality/gh-182-repair/`, using the same
writable Cargo environment listed above. No production logic, policy threshold,
workflow requirement, or OpenSpec task scope changed.

All five required local commands were rerun after the repair: nextest passed
354 tests (0 skipped), Python unittest passed 329 tests, formatting and Clippy
passed, and documentation consistency reported `pass`. The two project-local
configuration commands remain not applicable because `flow.toml` is absent.
The [repair evidence archive](https://github.com/musutrade/Harness-Gate/blob/d9a932b23dcfea709175285b594ca200450a9971/docs/quality/gh-182/repair-evidence.tar.xz) retains the complete logs,
completed failed-CI log, Windows reproduction wrapper and 39-case result, and
exact validation commands. Hosted acceptance for the repair is pending.
