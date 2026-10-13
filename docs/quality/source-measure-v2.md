# GH-94 source measurement series 2

This development-only series resolves the missing closure observations recorded
in [the preserved draft](gh-94-draft-validation.md). It does not alter shipped
instrumentation or replace the reproducible version 1 reports. The operator
authorized the analyzer, mapping, instrumentation and specification changes for
GH-94 on 2026-09-07. The [acceptance record](gh-94-validation.md) identifies the
measured source trees and results.

## Versioned contract

[`source_measure.py`](../../tools/quality/source_measure.py) uses the MIT-licensed
[`harness-gate-rust-measure`](https://github.com/musutrade/GateProbes/blob/155b233fb2f525f520b4fcc0ed73e017154345e0/plugins/rust-measure/Cargo.toml) 0.2.0
helper, with exact dependencies and a committed Cargo lockfile. Its series is:

| Dimension | Identity |
| --- | --- |
| AST analyzer | `harness-gate-rust-measure/0.2.0` |
| Complexity | `mccabe-rust-2/1` |
| Instrumentation | `closure-black-box/1` |
| Inverse source map | `insertions-utf8/1` |

Both reports record identical analyzer executable and implementation digests,
Rust/LLVM toolchain identity, original source digests, manifest digests and raw
LLVM export digests. Comparison rejects different series or implementation
identities. Version 1 and version 2 numbers are not comparable.

`syn` parses source functions, methods, default trait methods, local functions
and closures, including expression-bodied closures and local constants. Each
function owns its decisions; nested function and closure bodies are excluded
from the enclosing function's CC. Test functions and `cfg(test)` modules are
excluded, while production declarations without LLVM observations are blockers.
Unsupported production macro grammars fail closed. Expression-list macro
arguments and the `json!` object/array grammar are parsed recursively, including
their closures and decisions. This differs deliberately from version 1, which
skipped macro arguments. Macro-generated items and expansion coverage regions
are unsupported, not silently assigned to an enclosing function.

The raw counters and formula are:

```text
CC = 1 + if + guards + while + for + loop + and_and + or_or
       + question_mark + match_decisions
match_decisions = sum(max(0, number_of_arms - 1) for each match)
```

`if` includes `if let` and let-else. Each match guard contributes one additional
decision. Empty matches contribute zero, preserving minimum CC 1. `match` and
`match_arms` remain raw audit counts; `closures` and `nested_functions` are
inventory counts, not additional decisions. Unlike the version 1 grammar, these
counts come from parsed AST nodes. The fixed tests lock CC 4/2/2/1 for an outer
function, nested function, macro-argument closure and excluded test; empty-match
and macro-argument-if fixtures lock CC 1/2. A combined control-flow fixture
locks every raw decision count and CC 14, including `?`, `||`, `if let`,
let-else, both while forms, for, loop, and guarded three-arm match.

## Distinguishing instrumentation and mapping

Only disposable snapshots are instrumented. Every production closure body
`EXPR` becomes `{ ::std::hint::black_box(()); EXPR }`. No original source bytes
are deleted or rewritten, and the insertion introduces no lines. The closure
still returns its original expression. The observable black-box call prevents
the compiler from erasing the closure's own coverage record in this toolchain.
This is an instrumented development build; performance numbers must not be
derived from it. The original product build runs the same behavior suite.

The executable fixture in
[`test_source_measure.py`](../../tools/quality/tests/test_source_measure.py)
runs `all(|step| step.passed)` with an empty input and one false element. LLVM
records the closure's own entry count as **0 versus 1**, respectively; its own
mapped original regions distinguish the two cases. This is the observation
that uninstrumented parent counters could not provide. The test also deletes
that LLVM record and requires a missing-evidence failure. No parent count is
ever assigned to a closure.

The manifest retains complete original source, AST inventory, insertion points
and original/instrumented digests. The mapper verifies the actual instrumented
file, reparses the original, and reproduces all edits before consuming counters.
AST character columns are converted to UTF-8 byte columns; LLVM coordinates are
mapped back by removing inserted bytes. Wholly synthetic, zero-width mapped
regions are removed. Original executable intervals remain in the denominator.
Nested/adjacent insertions and non-ASCII source have fixed mapping tests.

Every original production symbol in all selected files must join to its
own LLVM function record. Every LLVM function record in those sources must
join to one innermost AST symbol or an explicitly excluded test symbol.
Demangled terminal closure names validate kind; a closure type inside a generic
function's type arguments does not make that function a closure. Unmatched,
ambiguous, cross-file, macro-expansion, source-mismatched or missing observations
block measurement. A real zero counter remains a zero observation.

Instantiations of one source function are unioned by maximum counter per source
region. LLVM code regions (kind 0) form the region denominator. The line sweep
uses innermost code intervals, excludes gap/skipped intervals and subtracts
nested symbols. Parent and closure coverage therefore remain independent. Raw
mapped regions, counts, instance names/indices, spans and source digests remain
in each report. Branch coverage remains **unsupported** in this series.

## Risk selection and source identity

The six original hotspots and every extracted named responsibility are listed
in `HOTSPOTS`, alongside `project/input.rs::allocate_snapshot_root`. All must have line **and** region coverage >=80% and
`crap_line <=30`; the comparison also rejects a missing extracted function.
CRAP is calculated using exact rational arithmetic:

```text
crap_line = CC + CC² × (1 - covered_lines / executable_lines)³
```

The existing OpenSpec incremental ratchet also applies to every independently
inventoried function, including closures: changed functions require CRAP <=30;
changed functions with CC >10 additionally require both coverage thresholds.
A closure does not inherit its parent's coverage or risk result. Unchanged
historical functions retain their measured debt. Reports distinguish the
three-threshold result (`passed`) from the incremental decision (`accepted`),
so accepted low-risk or unchanged debt never becomes a coverage pass.

Exact parsed token-sequence SHA-256 identifies unchanged code across line moves
and closure reparenting. Matching is a multiset within source file and function
kind: duplicate identical bodies consume one old identity each in source order.
This proves identical source occurrences, not unique semantic lineage between
identical copies. Renames or edited tokens conservatively count as changed.
The original/extracted responsibility table records decomposition separately;
new functions cannot borrow an old high-risk baseline. Both complete symbol
inventories and unmatched old identities are retained.

The helper covers the six GH-94 source files and `project/input.rs`. The
selection identity `gh94-and-staged-snapshot/1` records the added snapshot
allocator. Both revisions must be remeasured with this selection; prior
six-file reports cannot be mixed into it. The allocator regression covers a
symlinked temporary parent, cleanup, and invalid temporary parents. All symbols
in the added file are inventoried and retain the same incremental ratchet.
The helper does not
certify the repository or turn existing version 1 blockers into passes.
Unsupported platforms, grammar and observations require a new tested series;
they are not threshold exceptions. CI installs LLVM tools to execute the
distinguishing fixture instead of skipping it when instrumentation is absent.

## Reproduction

Run from the repository root, using a fresh directory below `target/quality`.
Build the same helper once for both snapshots:

```bash
CARGO_TARGET_DIR="$PWD/target/gh-94-measure" cargo build \
  --manifest-path tools/quality/rust-measure/Cargo.toml --locked
```

Archive base `764482ca64c754cdb56e3a3f97bc267023497736` and the PR head using
`git archive`, each into a separate workspace-local snapshot. Do not check out
or modify another workspace. For each snapshot, substitute its path for
`SNAPSHOT`, and `base` or `head` for `LABEL`:

```bash
python3 tools/quality/source_measure.py prepare \
  --crate SNAPSHOT/tools/harness-gate \
  --binary target/gh-94-measure/debug/harness-gate-rust-measure \
  --manifest target/quality/LABEL-manifest.json
CARGO_TARGET_DIR="$PWD/target/gh-94-coverage" cargo llvm-cov nextest \
  --manifest-path SNAPSHOT/tools/harness-gate/Cargo.toml --locked \
  --json --output-path target/quality/LABEL-coverage.json
python3 tools/quality/source_measure.py measure \
  --crate SNAPSHOT/tools/harness-gate \
  --binary target/gh-94-measure/debug/harness-gate-rust-measure \
  --manifest target/quality/LABEL-manifest.json \
  --llvm target/quality/LABEL-coverage.json \
  --output target/quality/LABEL-risk.json
```

Collect sequentially; `cargo llvm-cov` cleans only this task-owned target before
each collection. The base measurement intentionally exits 1 with its measured
hotspot failures; inspect the generated report. A mapper error instead prevents
report generation and blocks comparison. Head measurement and comparison must
exit 0:

```bash
python3 tools/quality/source_measure.py compare \
  --base target/quality/base-risk.json --head target/quality/head-risk.json \
  --output target/quality/comparison.json
python3 -m unittest discover -s tools/quality/tests -v
```
