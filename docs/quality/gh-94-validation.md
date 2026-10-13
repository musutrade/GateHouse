# GH-94 local validation and delivery evidence

Tasks 6.1–6.6 are locally validated. **CI and controller acceptance remain pending.**
This record certifies the 32 selected functions in six files, not the whole
OpenSpec proposal or a replacement repository baseline.

## Source and history

- Base: `764482ca64c754cdb56e3a3f97bc267023497736` (GH-93).
- Final implementation: `a1cc257586393a3cf8b88ab192d131b91d20e188`.
  Subsequent evidence/documentation changes do not change the measured source.
- Preserved remote draft: `41bba0035ca24c42ab86fe7b18249459cfc43f61` and all
  its ancestors. Local changes were backed up as a patch, documentation archive,
  Git bundle and two retained stashes under `target/quality/gh-94/backups` / Git.
  After verifying the draft's matching source, local Git fast-forwarded to it.
  Normal `git add` and `git commit` succeeded; there was no reset or force push.
- GitHub API confirmed #92 and #93 are closed with `state_reason=completed` on
  2026-09-07. GH-94 already has `symphony-ready`.

The [collection manifest](https://github.com/musutrade/Harness-Gate/blob/d9a932b23dcfea709175285b594ca200450a9971/docs/quality/gh-94-v2/collection.json) verifies all six original
source hashes against both commits, plus Cargo files and the relevant CLI and
failure-path tests. Complete original source and AST inventories are retained
in compressed manifests, alongside raw LLVM exports and collection logs.
The task-owned coverage target was cleaned between snapshots, so prior base
binaries are not retained; the collected source, export and tool digests remain.
No result here claims cross-platform or branch coverage.

## Decomposition and behavior

Doctor dispatch delegates each check kind to its own validation function.
CLI handlers retain the shared error exit. Verification separates service
results, ordered result merging and report publication. Configured tasks
separate runner, isolation, shard and environment construction. Adapter
preflight, process coordination, waiting, output validation and response
validation have distinct responsibilities. JSON counting separates explicit
paths from supported recursive discovery.

The adapter waiter directly uses the reaped status returned by `terminate`;
the draft's redundant termination/`try_wait` wrapper was removed. Tests cover
writer errors/disconnection, observed stdout/stderr limits, output precedence
and combined budgets. Additional CLI tests cover text/JSON dispatch, sealed
hook snapshots, cleanup resources, compatibility request identity and shadow
failure. Existing gate order, cancellation, timeout, cleanup/report precedence,
lease ownership and strict JSON regressions remain green.

## Comparable function evidence

The operator-authorized [series 2 contract](source-measure-v2.md) resolves both
frozen-parser failures and missing expression-closure observations. Both base
and head use the same locked AST helper, CC rules, insertion-only
instrumentation and inverse UTF-8 mapping, with matching implementation and
executable digests. It measures real closure counters; it never substitutes
parent counters. A compiled fixture observes zero on empty input and one on a
single false element, and rejects a deleted record or altered source.

There are **198 base and 212 head production symbols**, including closures.
Every symbol in the selected files has its own LLVM observation. All **32
selected functions** meet line AND region coverage >=80% and `crap_line <=30`.
The incremental comparison checks **35 changed identities** with no failures.
Base measurement exits 1 for its actual failing hotspots; head measurement and
comparison exit 0. No missing observation is treated as a zero or a pass.

The table shows the worst result among selected head functions in each file;
threshold comparisons use exact counts and rational CRAP, not these rounded
presentation values.

| Source | Base parent CC / CRAP | Head selected | Max CC | Min line | Min region | Max CRAP |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| `doctor/checks.rs` | 21 / 21.03 | 6 | 9 | 90.91% | 92.86% | 9.00 |
| `app/commands.rs` | 59 / 323.61 | 8 | 19 | 88.24% | 81.25% | 19.21 |
| `verify/mod.rs` | 33 / 36.27 | 4 | 22 | 88.71% | 83.77% | 22.70 |
| `verify/steps.rs` | 13 / 13.00 | 5 | 5 | 97.83% | 94.87% | 5.00 |
| `verify/parser.rs` | 12 / 12.00 | 3 | 11 | 93.33% | 93.48% | 11.04 |
| `process/adapter.rs` | 55 / 113.35 | 6 | 16 | 80.95% | 84.62% | 16.03 |

The complete [base report](https://github.com/musutrade/Harness-Gate/blob/d9a932b23dcfea709175285b594ca200450a9971/docs/quality/gh-94-v2/base-risk.json),
[head report](https://github.com/musutrade/Harness-Gate/blob/d9a932b23dcfea709175285b594ca200450a9971/docs/quality/gh-94-v2/head-risk.json) and
[source identity comparison](https://github.com/musutrade/Harness-Gate/blob/d9a932b23dcfea709175285b594ca200450a9971/docs/quality/gh-94-v2/comparison.json) retain every function,
span, decision count, raw region/counter, exact CRAP and decomposition mapping.
The base export was reused only after verifying that the final AST metadata
produces exactly the same instrumentation edits and source bytes. The final
head export uses all 312 tests and the simplified waiter.

**Coverage debt remains explicit:** 75 nonselected head symbols fail at least
one of the three thresholds. Of these, 73 are unchanged exact-token identities;
two changed CC=1 error closures have observed zero line/region coverage and
CRAP=2: `count_json_path::closure_242_26` (u64-to-usize overflow, untriggerable on
this 64-bit target) and `wait_for_adapter::closure_474_44` (termination error).
They pass only the existing low-risk incremental CRAP rule; their coverage
results remain false. No parent coverage is assigned to them and no exception
changes a selected function's threshold. Their raw observations and identities
are in the same report, rather than an omitted/unmeasured category.

## Validation commands and results

All commands ran from this workspace. Rust builds use workspace-local targets.

| Command | Actual result |
| --- | --- |
| `CARGO_TARGET_DIR=target/gh-94-validation cargo nextest run --manifest-path tools/harness-gate/Cargo.toml --locked` | 312 passed, 0 skipped; [log](https://github.com/musutrade/Harness-Gate/blob/d9a932b23dcfea709175285b594ca200450a9971/docs/quality/gh-94-v2/final-nextest.log) |
| `cargo fmt --manifest-path tools/harness-gate/Cargo.toml -- --check` | Exit 0 |
| `CARGO_TARGET_DIR=target/gh-94-validation cargo clippy --manifest-path tools/harness-gate/Cargo.toml --all-targets -- -D warnings` | Exit 0; [log](https://github.com/musutrade/Harness-Gate/blob/d9a932b23dcfea709175285b594ca200450a9971/docs/quality/gh-94-v2/final-clippy.log) |
| `python3 -m unittest discover -s tools/quality/tests -v` | 92 passed; includes real LLVM distinguishing/negative fixtures; [log](https://github.com/musutrade/Harness-Gate/blob/d9a932b23dcfea709175285b594ca200450a9971/docs/quality/gh-94-v2/quality-tests.log) |
| `CARGO_TARGET_DIR=target/gh-94-measure cargo clippy --manifest-path tools/quality/rust-measure/Cargo.toml --all-targets --locked -- -D warnings` | Exit 0; [log](https://github.com/musutrade/Harness-Gate/blob/d9a932b23dcfea709175285b594ca200450a9971/docs/quality/gh-94-v2/analyzer-clippy.log) |
| `cargo fmt --manifest-path tools/quality/rust-measure/Cargo.toml -- --check` | Exit 0 |
| `CARGO_TARGET_DIR=target/gh-94-validation python3 tools/quality/contracts.py --output target/quality/gh-94/final-contracts.json` | All 20 scenarios and unchanged textual golden snapshot pass; [report](https://github.com/musutrade/Harness-Gate/blob/d9a932b23dcfea709175285b594ca200450a9971/docs/quality/gh-94-v2/final-contracts.json) |
| `CARGO_TARGET_DIR=target/gh-94-validation python3 tools/quality/docs_consistency.py --output target/quality/docs-consistency.json` | Exit 0; [report](https://github.com/musutrade/Harness-Gate/blob/d9a932b23dcfea709175285b594ca200450a9971/docs/quality/gh-94-v2/docs-consistency.json) |
| `openspec validate strict-json-results-and-risk-based-quality-gates --strict` | Exit 0; [log](https://github.com/musutrade/Harness-Gate/blob/d9a932b23dcfea709175285b594ca200450a9971/docs/quality/gh-94-v2/final-openspec.log) |
| `harness-gate config check` | Not applicable: `.harness-gate/flow.toml` is absent |
| `harness-gate verify --profile ci --all` | Not applicable: no project-local config or declared `ci` profile |

The CLI contract tool's fixed binary path used a temporary workspace-local symlink to the
same freshly built `target/gh-94-validation` used by nextest; the symlink was then
removed. No old binary or
accepted snapshot was substituted. Exact base/head collection commands, tool
versions and artifact hashes are in the collection manifest; reproduction is
in the versioned contract. The coverage runs pass 308 base / 312 head tests.

The initial docs-consistency invocation omitted `CARGO_TARGET_DIR` and failed
its Cargo-backed checks because the environment default target was read-only.
The reproducing command `cargo run --manifest-path tools/harness-gate/Cargo.toml
--locked -- config schema` exited 101 before CLI execution: `failed to open:
/home/gem/cargo-target/debug/.cargo-build-lock`, caused by `Read-only file system
(os error 30)`. The [original error](https://github.com/musutrade/Harness-Gate/blob/d9a932b23dcfea709175285b594ca200450a9971/docs/quality/gh-94-v2/docs-cargo-error.log) is retained.
Setting the workspace-local target resolved this dependency; the final report
passes all schema, example, migration, language, link and sandbox-wording checks.

## Preserved diagnostics and remaining scope

The [original draft validation](gh-94-draft-validation.md),
[parser failures](https://github.com/musutrade/Harness-Gate/blob/d9a932b23dcfea709175285b594ca200450a9971/docs/quality/gh-94-measurement-errors.json),
[native closure gap](https://github.com/musutrade/Harness-Gate/blob/d9a932b23dcfea709175285b594ca200450a9971/docs/quality/gh-94-coverage-gap.json) and their linked probes remain
unchanged. They describe the superseded blocked draft, not passing version 1
measurements. The original Git sandbox failure no longer reproduces after the
operator made this task's `.git` writable. Other draft probes were not rerun.

Version 1 tooling and historical baselines are unchanged. Series 2 is scoped
to these six files and rejects unsupported grammar/observations. Broader gates,
source traceability and baseline adoption in tasks 7–9 remain unchecked.
The existing PR #105 and branch are reused. The controller owns required CI,
squash merge and issue closure after handoff; local validation is not delivery.
