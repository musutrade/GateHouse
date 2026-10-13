# Complexity analyzer and quality evidence contract

This page records the locked development complexity analyzer and the
machine-readable evidence it emits. It is the written contract behind
OpenSpec tasks 1.2 and 1.3 in
[`strict-json-results-and-risk-based-quality-gates`](../../openspec/changes/strict-json-results-and-risk-based-quality-gates/proposal.md):
the analyzer choice, license, syntax limits, and frozen fixture are fixed so
that raw complexity evidence can be reproduced, compared, and reviewed. This
contract changes quality tooling only; it does not change Harness-Gate
runtime behavior.

## Locked analyzer

| Field | Frozen value |
| --- | --- |
| Name | `harness-gate-complexity` |
| Version | `0.1.1` |
| License | MIT |
| Runtime | Python standard library only |
| Use | Development/CI quality evidence; never linked into the release binary |

The analyzer is intentionally a small in-repository Python program
([`complexity_analyzer.py`](../../tools/quality/complexity_analyzer.py))
rather than a third-party Rust/Python analyzer. This keeps the quality-script
CI job free of extra installs and makes every fixture rebuild deterministic
from the checked-in source alone. Its MIT license matches the repository's own
license, and its input subset, raw counts, and error behavior are frozen in
the repository instead of following an upstream analyzer release. Using the
analyzer is not an OS sandbox and does not claim any stronger isolation than
the quality CI job already has.

The analyzer reads UTF-8 Rust fixture text lexically and never compiles or
executes the fixture; Rust does not need to be installed for reproduction.

Version 0.1.1 fixes attribute/restricted-visibility scanning, type-qualifier
progress, zero-argument closure recognition, and function spans that include
visibility/modifier prefixes. The counting rule remains `mccabe-rust-1` version
1. These fixes create a new analyzer series: retained 0.1.0 baselines must not
be rewritten or compared as compatible evidence. The frozen controls retain
their previous counts. [Function risk evidence](function-risk.md) joins this
output to LLVM source locations and rejects unsupported or missing mappings.

## Locked rule and cyclomatic complexity formula

The analyzer applies one frozen rule:

| Field | Frozen value |
| --- | --- |
| Rule name | `mccabe-rust-1` |
| Rule version | `1` |
| Rule license | MIT |

For each analyzed symbol, cyclomatic complexity is recomputed from raw counts
as:

```text
1
+ raw.if
+ raw.guards
+ raw.while
+ raw.for
+ raw.loop
+ max(0, raw.match_arms - raw.match)
+ raw.and_and
+ raw.or_or
+ raw.question_mark
```

The `closures`, `macro_invocations_skipped`, and `nested_functions` counters
describe symbol structure and remain in the raw evidence, but they do not add
to complexity by themselves. Closure bodies are scored as their own symbols
(see [Closures](#closures)), and nested function bodies are excluded from the
enclosing function's counts (see [Nested functions](#nested-functions)).

The validator recomputes this value from the raw counts and rejects any record
whose `metrics.cyclomatic_complexity` does not match
([`quality_evidence.py`](../../tools/quality/quality_evidence.py)).

## Frozen syntax subset and explicit limits

The locked fixture must stay inside the following lexical subset:

- Item `fn` definitions at file, `mod`, `trait`, and `impl` scope. Other item
  keywords (`struct`, `enum`, `union`, `use`, `type`, `static`, `trait`,
  `impl`, `mod`, and `macro_rules`) are recognized for skipping, not analyzed
  as symbols.
- `if`, `if let`, `else if`, and ordinary `{ ... }` blocks.
- `match` expressions with counted arms and per-arm `if` guards.
- `while`, `while let`, `for`, and `loop`.
- The `&&` and `||` short-circuit operators and the postfix `?` operator.
- Closures with a braced body: `|...| { ... }`, `|| { ... }`, and
  `move |...| { ... }`.
- Macro invocations whose token content is skipped and counted once.

Anything outside that subset raises `ComplexitySyntaxError` instead of
producing partial evidence. This is a fail-closed contract: a fixture that
reaches syntax the analyzer cannot score is an error, never a silent guess.
For example, expression-bodied closures (`|a, b| a + b`) and unknown
expression tokens are rejected, and the analyzer exits non-zero without
writing a record. The schema and tests cover the fail-closed cases so a
future analyzer change cannot quietly broaden the frozen subset.

## Construct handling

### `if`, guards, and loops

Each `if`/`if let`/`else if` keyword increments `raw.if`. A match arm guard
such as `0 if value == 0 =>` increments `raw.guards`. `while` and `while let`
increment `raw.while`, `for` increments `raw.for`, and `loop` increments
`raw.loop`. Short-circuit `&&`/`||` increment `raw.and_and`/`raw.or_or`, and
each `?` increments `raw.question_mark`.

### `match`

Each `match` expression increments `raw.match`, and each arm increments
`raw.match_arms`. The complexity contribution is the number of arms beyond
the first match expression, `max(0, raw.match_arms - raw.match)`. The
validator enforces `raw.match_arms >= raw.match`, `raw.match_arms == 0` when
`raw.match == 0`, and `raw.guards <= raw.match_arms`.

### Closures

Only braced closures are supported. Each braced closure increments the raw
`closures` counter of the enclosing symbol, and its body is scored under its
own symbol named `<function>::closure_<n>` where `<n>` is declaration order
within that function (for example `query::closure_1`,
`call_double::closure_1`, and `call_double::closure_2`). Counts inside the
closure body belong to the closure symbol, so `call_double` has complexity 1
even though its first closure contains an `if`.

### Nested functions

A nested `fn` item inside a function is counted in that function's raw
`nested_functions` counter, but its body is excluded from every symbol and it
does not receive its own symbol. The frozen fixture's `outer` therefore has
complexity 1 while its `inner` helper contains an `if` that is deliberately
not scored.

### Macros

A macro invocation is skipped at one level and counted once in
`macro_invocations_skipped`; its tokens are not analyzed for control flow.
`macro_rules!` definitions are recognized as items and are not scored. The
frozen fixture's `with_macros` records `macro_invocations_skipped: 2` for its
`format!` and `assert!` calls and has complexity 1.

## Frozen fixture

The locked sample is
[`tools/quality/fixtures/complexity/controls.rs`](../../tools/quality/fixtures/complexity/controls.rs)
with committed expected evidence in
[`controls.expected.json`](../../tools/quality/fixtures/complexity/controls.expected.json).
The source is 1,771 UTF-8 bytes with SHA-256
`e049647d1177f5bbbb2bd2f73cf11169722544a571b16fbc3aef823a393e3bb8`. Its 11
symbols and locked complexity values are:

| Symbol | Cyclomatic complexity |
| --- | ---: |
| `decision` | 5 |
| `classify` | 5 |
| `loops` | 5 |
| `query` | 2 |
| `query::closure_1` | 1 |
| `call_double` | 1 |
| `call_double::closure_1` | 2 |
| `call_double::closure_2` | 1 |
| `outer` | 1 |
| `with_macros` | 1 |
| `main` | 1 |

These values reproduce from the documented commands below; the committed
expected JSON omits the optional `commit` field so the fixture is comparable
across checkouts.

## Versioned evidence schema

The canonical shape is
[`tools/quality/schema/quality-evidence.schema.json`](../../tools/quality/schema/quality-evidence.schema.json):

- `schema_version` is `"1"` and `kind` is `"complexity-evidence"`.
- `series` records the analyzer and rule name/version/license, the toolchain
  target and Python version, and the source path, SHA-256, byte count, and
  language.
- Each `symbol` records `id`, `kind`, `qualified_name`, `source`, `span`, raw
  counts, and the recomputed `metrics.cyclomatic_complexity`.
- `commit` is an optional 40-hexadecimal current commit recorded by the
  generator when `git rev-parse HEAD` succeeds; the committed fixture omits it
  so the fixture is not tied to one checkout.
- Unknown fields and non-canonical values are rejected by the validator.

The validator is stdlib-only and deterministic
([`quality_evidence.py`](../../tools/quality/quality_evidence.py)): it checks
the schema subset, canonical series keys and symbol identities, digest/byte
agreement, raw-count constraints, and complexity recomputation. All checks
fail closed, including missing raw keys, negative counts, duplicate or
non-canonical symbol IDs, and inconsistent series.

## Series identity

The canonical series key concatenates these components with `|`:

```text
kind
| analyzer.name
| analyzer.version
| rule.name
| rule.version
| toolchain.target
| toolchain.python
| source.path
| source.sha256
| source.bytes
```

No component may be empty or contain `|` or a newline. `compare-series`
requires every record to produce the same key; any change of analyzer, rule,
target/Python toolchain, source path, source digest, or source byte count
makes records incompatible and comparison fails closed instead of silently
mixing baselines. A different rule or analyzer version therefore starts a new
series.

## Source-symbol identity

Each symbol carries a canonical identity:

```text
source_path::kind::qualified_name@start_line:start_column:source_sha256[:12]
```

For example:

```text
controls.rs::function::decision@7:1:e049647d1177
```

The identity includes the relative source path, symbol kind, qualified name,
start position, and a digest prefix, so names alone cannot match symbols from
different files or moved/rewritten source. The validator rejects any `id`
that is not exactly this canonical form and rejects duplicate ids.

## Regenerating and validating evidence

Run from the repository root. Generate candidate evidence into the ignored
`target/quality` directory:

```bash
python3 tools/quality/complexity_analyzer.py \
  --source tools/quality/fixtures/complexity/controls.rs \
  --source-root tools/quality/fixtures/complexity \
  --output target/quality/complexity/controls.json
```

Validate a record and compare series:

```bash
python3 tools/quality/quality_evidence.py validate \
  --record target/quality/complexity/controls.json
python3 tools/quality/quality_evidence.py validate \
  --record tools/quality/fixtures/complexity/controls.expected.json \
  --expected-source-sha256 e049647d1177f5bbbb2bd2f73cf11169722544a571b16fbc3aef823a393e3bb8 \
  --expected-source-bytes 1771
python3 tools/quality/complexity_analyzer.py \
  --source tools/quality/fixtures/complexity/controls.rs \
  --source-root tools/quality/fixtures/complexity \
  --output target/quality/complexity/controls-again.json
python3 tools/quality/quality_evidence.py compare-series \
  --record target/quality/complexity/controls.json \
  --record target/quality/complexity/controls-again.json
```

Compare with the committed expectation after normalizing the machine-specific
toolchain fields (`toolchain.target`, `toolchain.python`) and removing
`commit`; the unit suite does this normalization. Raw counts, source digest,
source byte count, spans, ids, and complexity must otherwise be byte-for-byte
identical. `compare-series` itself requires identical toolchain metadata, so
records generated on different machines/toolchains are correctly reported as
incompatible rather than compared.

The frozen-fixture and fail-closed tests run with:

```bash
python3 -m unittest discover -s tools/quality/tests -v
python3 -m py_compile tools/quality/*.py tools/quality/tests/*.py
```

## Reproducibility expectations

On the same checkout and Python/toolchain, regenerating the record must be
stable: same symbols in declaration order, same raw counts and complexity,
same spans, same source digest and bytes, and the same optional `commit` when
the working tree is unchanged. The expected fixture and tests lock this
behavior so the acceptance criteria for task 1.2 and 1.3 have concrete
evidence rather than an informal tool description.
