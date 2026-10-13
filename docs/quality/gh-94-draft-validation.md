# GH-94 draft implementation evidence

Scope: OpenSpec tasks 6.1–6.6 in
[`strict-json-results-and-risk-based-quality-gates`](../../openspec/changes/strict-json-results-and-risk-based-quality-gates/tasks.md).
Dependencies #92 and #93 were closed with `state_reason=completed` before work
started. **Acceptance is blocked by production function measurement errors.**
The tasks remain unchecked. This draft is not a production risk baseline or a
completed delivery.

## Changes and compatibility

| Original hotspot | Extracted responsibilities |
| --- | --- |
| `doctor/checks.rs::run_check` | Path, environment, environment/file, Git configuration and version checks |
| `app/commands.rs::run` | Doctor, cleanup, scope, secrets, audit, verify and hook handlers; existing `CliError` return contract retained |
| `verify/mod.rs::run_selected` | Service status, ordered result normalization and report publication |
| `verify/steps.rs::configured_task` | Runner construction, isolation allocation, shard metadata and final environment injection |
| `process/adapter.rs::run_with_cancel` | Request preparation, process execution/waiting, repeated termination/reaping, output budgets and response validation |
| `verify/parser.rs::count_json_results` | Explicit-path counting and recursive recognized-result discovery |

The verification coordinator retains planning/waiver checks before allocation,
cleanup before publication, and cancellation/primary/cleanup error precedence.
Adapter waiting retains child-exit, stdin failure, stdout overflow, stderr
overflow, cancellation and timeout ordering. Reader/writer deadlines share the
same elapsed budget. Runner environment construction retains service injection
before explicit removals. JSON traversal and valid-input count semantics are
unchanged. The Issue #93 regression suite ran before and after the changes.

## Blocking evidence

The unchanged analyzer from #92 is `harness-gate-complexity 0.1.1`, rule
`mccabe-rust-1` version 1. It fails on **all six selected source files at both
base and head**. See the committed
[commands, exit statuses and source SHA-256 identities](https://github.com/musutrade/Harness-Gate/blob/d9a932b23dcfea709175285b594ca200450a9971/docs/quality/gh-94-measurement-errors.json).
All six retained base files were verified byte-for-byte against Git commit
`764482ca64c754cdb56e3a3f97bc267023497736`. Head hashes identify the draft's
production source bytes, independently of the documentation commit.

For example:

```bash
python3 tools/quality/complexity_analyzer.py \
  --source tools/harness-gate/src/doctor/checks.rs \
  --source-root tools/harness-gate \
  --output target/quality/gh-94/head-doctor-checks.rs.complexity.json
```

Exit 1: `src/doctor/checks.rs:34:28: non-braced closure body is outside the
frozen syntax subset (braced closures only)`.

Other failures include `expected an item, found 'const'` in task construction
and adapter code, and `unterminated item` in verification orchestration. These
failures precede the refactor. A temporary analyzer probe was extended to parse all six selected base/head
files. Its unversioned output is not accepted evidence, and the frozen analyzer
is unchanged. Parsing support alone does not resolve the mapping failures below.

The [frozen analyzer contract](complexity-analyzer.md) and
[function mapping contract](function-risk.md) prohibit silently dropping
unsupported symbols or mixing measurement series. Consequently this draft
does **not** claim function line/region coverage >=80%, `crap_line <=30`,
complete old/new function identity mapping, or an enumerated historical debt
baseline. Those are still required for every task's acceptance. Module or suite
coverage cannot substitute for the missing function evidence.

To unblock acceptance, the production syntax and source-location mapping must
be supported by a versioned, fixture-validated analyzer. Both base and head
must then be measured with that same series, all extracted functions must be
checked individually, and historical debt must be reported separately. The
frozen risk tooling and its thresholds are unchanged by this draft.

## Follow-up: independently reproduced coverage gap

A standalone Rust program now reproduces a second blocker independently of the
analyzer. The [complete source, exact commands, toolchain and raw LLVM export](https://github.com/musutrade/Harness-Gate/blob/d9a932b23dcfea709175285b594ca200450a9971/docs/quality/gh-94-coverage-gap.json)
are retained together. All four commands exited 0; the executable asserts both
true and false outcomes for each of these closures:

```rust
fn field_only(steps: &[Step]) -> bool { steps.iter().all(|step| step.passed) }
fn method_call(steps: &[String]) -> bool { steps.iter().all(|step| step.is_empty()) }
```

With `rustc -C instrument-coverage -C opt-level=0`, LLVM exports four functions:
`main`, both named functions, and the `method_call` closure. It exports **no
function record for the `field_only` closure**, despite executing it. The same
absence occurs in the retained production exports for
`verify/mod.rs::run_selected` (`|step| step.passed`, base line 270/head line 282)
and `verify/steps.rs::run_configured_step` (`|policy| policy.backoff_ms`, line 93).
This record reports absence, without inferring an optimizer cause or assigning
coverage to the missing function.

The probe also encounters LLVM closures inside macros in `app/commands.rs`
(`println!`, base line 247/head line 270) and `process/adapter.rs` (`format!`,
base line 450/head line 574), whose token bodies the frozen analyzer explicitly
skips. Expression-closure region endpoints can also precede closing call
parentheses, so the frozen exact-end source join cannot accept them. These are
separate mapping requirements, not merely missing lexer syntax.

The probe was not promoted: accepting it would require a versioned, fixture-tested
syntax/mapping contract and explicit review of unmeasurable ranges. The current
contract requires missing production functions to fail closed. No closure was
silently dropped, no missing counter was synthesized, and no baseline or
threshold was changed. GH-92's own validation record explicitly deferred
production hotspot evidence; its closed tracker state supplies no such evidence.

Reproduce the standalone program by writing the artifact's `source.text` to its
`source.path`, then running its four recorded commands. The raw profiles and
binary remain under `target/quality/gh-94/closure-reproducer/`. This is diagnostic
evidence for the blocker, not a passing function-risk report. Tasks 6.1–6.6 and
final handoff remain incomplete until the production measurement contract can
represent these cases and both revisions pass the unchanged thresholds.

The continuation also tested three compiler settings against the same retained
reproducer: `-C instrument-coverage=all`, `-C instrument-coverage -C
link-dead-code=yes`, and `-C instrument-coverage -C no-prepopulate-passes`, each
with `-C opt-level=0`. All twelve compile/run/merge/export commands succeeded,
but every export still contains the same four function identities and omits the
field-only closure. The diagnostic JSON now retains each exact command, profile
environment, exit status, export digest and complete raw export under
`instrumentation_probes`. These options do not unblock the frozen mapper.
Rust's [instrumentation documentation](https://doc.rust-lang.org/rustc/instrument-coverage.html)
also describes `all` as a current alias for enabled instrumentation, rather than
an option promising additional closure records.

## Follow-up: parent counters cannot recover missing closure coverage

A paired-run probe uses one instrumented executable and identical source bytes.
Calling `field_only` with an empty slice invokes its closure zero times and
returns true; calling it with one false entry invokes the closure once and
returns false. Both runs assert the expected result. LLVM emits exactly one
`field_only` record in each run, with identical function and region counters,
and no closure record. The function is marked `#[inline(never)]` and its input
passes through `std::hint::black_box`.

All nine tool commands passed. The source, commands, result assertions and both
raw exports are retained in `indistinguishable_execution_probe` in the
[coverage diagnostic](https://github.com/musutrade/Harness-Gate/blob/d9a932b23dcfea709175285b594ca200450a9971/docs/quality/gh-94-coverage-gap.json), with local artifacts under
`target/quality/gh-94/closure-distinguish/`. This establishes that using parent
counters to fill the missing closure record can report executed coverage for
an unexecuted closure. A syntax or mapper repair alone cannot supply the
missing observation. Acceptance still needs instrumentation that distinguishes
these executions, alongside the versioned syntax/mapping repair; the existing
source identity and per-function coverage requirements remain in force.

Documentation consistency and strict OpenSpec validation both passed after
this diagnostic addition (`docs-consistency-distinguish.log` and
`openspec-distinguish.log`). Production code is unchanged; its existing Rust
validation results still apply.

## Validation

Complete local logs and raw exports are retained under `target/quality/gh-94/`.
`CARGO_TARGET_DIR=target/gh-94-cargo` keeps builds inside this workspace.

| Exact command | Actual result |
| --- | --- |
| `CARGO_TARGET_DIR=target/gh-94-cargo cargo nextest run --manifest-path tools/harness-gate/Cargo.toml --locked` | PASS before refactoring: 308 tests, 0 skipped (`baseline-nextest.log`); PASS on final source: 308 tests, 0 skipped (`nextest-final.log`) |
| `cargo fmt --manifest-path tools/harness-gate/Cargo.toml -- --check` | PASS (`fmt.log`) |
| `CARGO_TARGET_DIR=target/gh-94-cargo cargo clippy --manifest-path tools/harness-gate/Cargo.toml --all-targets -- -D warnings` | PASS (`clippy.log`) after removing two redundant borrows introduced by extraction |
| `python3 -m unittest discover -s tools/quality/tests -v` | PASS: 85 tests (`quality-tests.log`) |
| `CARGO_TARGET_DIR=target/gh-94-cargo python3 tools/quality/docs_consistency.py --output target/quality/docs-consistency.json` | PASS: examples, migration, links and schemas (`docs-consistency-final.log`); follow-up diagnostic documentation also passes (`docs-consistency-followup.log`) |
| `openspec validate strict-json-results-and-risk-based-quality-gates --strict` | PASS (`openspec.log`); follow-up also valid (`openspec-followup.log`) |
| `CARGO_TARGET_DIR=target/gh-94-coverage cargo llvm-cov nextest --manifest-path tools/harness-gate/Cargo.toml --locked --json --output-path target/quality/gh-94/base-coverage.json` | PASS: baseline instrumented suite, 308 tests, 0 skipped (`base-coverage.log`); 467 raw profiles retained in `base-profiles/` |
| `CARGO_TARGET_DIR=target/gh-94-coverage cargo llvm-cov nextest --manifest-path tools/harness-gate/Cargo.toml --locked --json --output-path target/quality/gh-94/head-coverage.json` | PASS: final instrumented suite, 308 tests, 0 skipped (`head-coverage.log`); raw LLVM export retained, not accepted function-risk evidence |
| `harness-gate config check` | NOT APPLICABLE: `.harness-gate/flow.toml` is absent |
| `harness-gate verify --profile ci --all` | NOT APPLICABLE: no project-local configuration declaring a `ci` profile |

The environment's inherited Cargo target directory is read-only. The first
unmodified docs-consistency invocation failed its Cargo-backed checks; using
the workspace target directory resolved them. No project configuration was
invented. During measurement investigation,
`python3 -m venv target/quality/gh-94/venv` also failed because `ensurepip is not
available` (the system suggests `python3.14-venv`). No alternate parser was
installed; this attempt supplies no measurement evidence.

Toolchain: Rust 1.97.1, LLVM `22.1.6-rust-1.97.1-stable`, Python 3.14.4,
target `x86_64-unknown-linux-gnu`. No macOS/Windows acceptance or complete branch
coverage is claimed. [ADR-0025](../adr/0025-phase-1-quality-baseline-gates.md)
and its historical measurements remain unchanged.

Local staging is also blocked by the workspace's read-only `.git` mount:
`git add` exits 128 with `Unable to create
'/home/gem/symphony-workspaces/GH-94/.git/index.lock': Read-only file system`.
The draft is published with the GitHub Git Data API on `symphony/GH-94`, using
the prepared baseline as its parent. Local Git metadata remains at the
baseline. No completion handoff is written for this unfinished work.
