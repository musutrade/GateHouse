# GH-134: real frontend acceptance window

Scope: [OpenSpec task 4.1](../../../openspec/changes/typescript-angular-reference-adapter/tasks.md),
following [ADR-0040](../../adr/0040-language-agnostic-evidence-policy.md).
The [acceptance runner](https://github.com/musutrade/Harness-Gate/blob/8d93ea53c7cf8b314bda816eafe13209da25c742/tools/quality/typescript_acceptance.py) creates
two local fixture Git histories and independently collects all four revisions
with the locked Angular/Node/TypeScript toolchain and live Rust provider.
Each collection performs two locked npm installs, client generation, provider
tests/build, Angular production build and Angular tests with native coverage.

| Pair | Actual test change | Native pricing line counters | Required 4/5 threshold and no regression |
| --- | --- | --- | --- |
| Compatible | Add another small-order test; retain the bulk-order test | 5/6 → 5/6 | pass; no debt |
| Regression | Remove the bulk-order test | 5/6 → 4/6 | fail; regression and new debt |

The regression's Angular tests still pass, but native coverage loses the actual
discount branch. Production source identity and measurement series remain stable.
At the additional 9/10 threshold, the compatible pair passes only because the
policy explicitly permits unchanged legacy debt; its ledger retains that debt.
The regression fails and retains regressed debt. Independent rational arithmetic
over native counters agrees with the generic engine's outcome, full ratchet
comparison and debt presence. [Results](https://github.com/musutrade/Harness-Gate/blob/d9a932b23dcfea709175285b594ca200450a9971/docs/quality/gh-134/results.json) retain both policies and
complete reports; no unexplained outcome, counter or debt mismatch remains.

Each snapshot retains native covered/total line and function counts for every
accepted file/function/method scope. All 88 counter comparisons pass. Replay compares those exact integers with
normalized evidence, using the independent Istanbul oracle. Percentages are
never used as a substitute for counters. Routes, templates, generated code and
unmeasured metrics retain their existing explicit capability states.

## Retained provenance and reproduction

[Window](https://github.com/musutrade/Harness-Gate/blob/d9a932b23dcfea709175285b594ca200450a9971/docs/quality/gh-134/window.json) records real fixture base/head commits, targets and run IDs.
`compatible.bundle` and `regression.bundle` preserve both Git histories. Each
snapshot directory contains `native.tar.gz` (manifest, commands, logs, source
bytes, raw Istanbul coverage and production/test source maps), parser index,
native counters, normalized evidence, command receipts and caller-pinned artifact digests. Replay
reconstructs the source-specific artifact directories referenced by normalized
evidence and reports. The tests
verify clean native revisions, parent lineage and every retained source file
against the corresponding Git commit. These fixture commits are separate from
the implementation delivery commit.

Replay needs Python and Git, with no npm install or live provider:

```bash
python3 tools/quality/typescript_acceptance.py --work target/quality/gh134/replay
python3 -m unittest discover -s tools/quality/tests -p 'test_typescript_acceptance.py' -v
```

For fresh native collection, use unused output directories and the fixture's
pinned runtime/tool versions:

```bash
python3 tools/quality/typescript_acceptance.py --collect --work target/quality/gh134/recollect --retained target/quality/gh134/recollected
```

Fresh history timestamps, paths and digests may differ. The expected counters,
policy outcomes and debt classifications must agree. The retained collection
command used `--work target/quality/gh134/native-v2` and the default retained path.

## Negative matrix

The [acceptance tests](https://github.com/musutrade/GateProbes/blob/a762f0c4c7da3945367b870bccc39aac8069132f/compatibility/legacy-core/snapshot/tools/quality/tests/test_typescript_acceptance.py)
also run the existing collector rejection suite against the fresh regression-head
archive, through internal and subprocess transports.

| Failure | Executable coverage and required disposition |
| --- | --- |
| Missing coverage | Remove a requested native file; measurement error |
| Duplicate/ambiguous symbols | Duplicate Istanbul function or parser symbol, unknown method or duplicate requested scope; measurement error |
| Stale/tampered sources and maps | Changed original bytes, stale map content, tampered map inventory, absent test maps, index digest/path escape; measurement error |
| Missing/invalid capabilities | Omitted capability or malformed request rejected; zero denominators remain unavailable |
| Unsupported required metrics | Branch coverage, cyclomatic complexity and CRAP block policy; no fabricated numeric defaults |
| Changed/incompatible series | Tool, normalization, source identity, runtime and rule versions block baseline reuse; existing Rust/TypeScript cross-series tests remain active |
| Failed collection/subprocess | Recorded failed command, incomplete collection, nonzero exit, timeout and malformed stdout; measurement error |
| Provenance mismatch | Rebound context/collector/scope, response provenance, undeclared or tampered artifacts; measurement error |
| Debt preservation | Unchanged debt remains visible; denied legacy debt fails; regression retains debt; missing/duplicate baseline blocks; incompatible head cannot rewrite historical evidence |

## Architecture and certification boundary

[TS-01/TS-02 decisions](../typescript-source-semantics.md) remain bounded to
unambiguous original TypeScript identity and validated transformation provenance.
TS-03 uses the existing per-record capability contract. The negative matrix
confirms these boundaries without changing generic schemas, runner or policy.
[TS-04's fail-closed contract freshness boundary](../gh-133/README.md) is unchanged.
No new architecture mismatch was found. Any future need beyond these boundaries
requires narrower certification or a separately reviewed generic contract delta.

This window establishes task 4.1 evidence only. It does not certify an adapter,
Angular templates/DI, browser execution, SSR, zoneless behavior, signals, RxJS,
router/platform behavior, branch coverage, complexity or CRAP. Advisory CI and
bounded certification/review remain tasks 4.2–4.3. Required hosted CI must pass
before merge and is pending at submission.

## Validation

Validation results and artifact checksums are recorded in `validation.json`;
`validation.tar.gz` retains complete command logs. The original 49 frontend tests
passed before implementation. Native Angular tests passed in each snapshot:
8 compatible-base, 9 compatible-head, 8 regression-base and 7 regression-head.

| Command | Actual result |
| --- | --- |
| `python3 tools/quality/typescript_acceptance.py --collect --work target/quality/gh134/native-v2` | Exit 0; four complete native runs, 15 successful commands each |
| `python3 tools/quality/typescript_acceptance.py --work target/quality/gh134/replay` | Exit 0; 88 exact counter comparisons, matching outcomes and debt |
| `python3 -m unittest discover -s tools/quality/tests -p 'test_typescript_acceptance.py' -v` | Exit 0; 22 passed |
| `cargo nextest run --manifest-path tools/harness-gate/Cargo.toml --locked` | Exit 0; 315 passed, 0 skipped |
| `cargo fmt --manifest-path tools/harness-gate/Cargo.toml -- --check` | Exit 0 |
| `cargo clippy --manifest-path tools/harness-gate/Cargo.toml --all-targets -- -D warnings` | Exit 0 |
| `python3 -m unittest discover -s tools/quality/tests -v` | Exit 0; 283 passed |
| `python3 tools/quality/docs_consistency.py --output target/quality/docs-consistency.json` | Exit 0; status `pass` |

Cargo commands use `CARGO_TARGET_DIR=$PWD/target` to keep build output inside this
workspace. The Python and documentation checks use the same environment for
their Cargo subprocesses.

`harness-gate config check` and `harness-gate verify --profile ci --all` are **not
applicable**: this checkout has no `.harness-gate/flow.toml`. No project-local
configuration was created.

The first collection completed its native commands but failed provenance review:
the reused helper's default working directory recorded the outer workspace
revision instead of the fixture commit. Those artifacts were discarded from the
acceptance window. The runner now explicitly selects the fixture directory and
all four runs were repeated; revision/source/lineage tests guard this distinction.
The initial log and manifest excerpts are retained in the validation archive.

`git add tools/quality/typescript_acceptance.py` exited 128 because the workspace's
`.git/index.lock` is read-only. Only this workspace's Git metadata was copied to
ignored `target/quality/gh134/delivery.git`, with no symlinks or external object
directories, for committing and pushing the same branch and working tree.
Original metadata remains at the baseline; the runtime handoff declares the
actual pushed commit.
