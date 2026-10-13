# Native Rust source inventory (GH-219)

`harness-gate-rust-measure --native-inventory source.rs` is an opt-in source
syntax inventory. Its identity is `harness-gate-rust-native-inventory/0.1.0`,
rule `source-decisions-native-1`. It emits `certified_llvm_mapping: false` and
cannot supply coverage or CRAP measurements. The default invocation retains the
legacy 0.3.0 reference grammar and output identity; binary hashes still change
and must be pinned when executing either mode. No stored evidence is rewritten.
The native identity is intentionally incompatible with `source_measure.py`.

This is the first stage of GH-215's measurement prerequisites: GH-219 supplies
source parsing, GH-220 must certify expanded production code and LLVM mapping,
and GH-221 must classify missing LLVM files using that mapping. Arc-Admin PR #39
remains a file-summary pilot. Its existing analyzer invocation is not switched.

## Supported syntax and limits

Fully qualified `tracing::{trace,debug,info,warn,error}`, corresponding `_span`
macros, `event` and `span` accept comma-separated expression fields, simple named
fields, target/parent directives and `%`/`?` formatting markers. The parser visits
value expressions, including decisions and closures. A formatting `?` is not the
Rust try operator; a `?` inside the value remains counted. This supports a bounded
subset of the [tracing grammar](https://docs.rs/tracing/latest/tracing/), not every
possible field spelling. Unparsed tokens fail rather than being discarded.

Fully qualified `tokio::select!` follows the documented
[branch syntax](https://docs.rs/tokio/latest/tokio/macro.select.html), including
`biased;`, futures, optional guards, handlers, block handlers without commas and
final `else`. Each expression retains its source span and closure ownership.
`select_decisions` counts alternatives minus one, plus syntactically refutable
patterns; `guards` counts preconditions. Wildcards, binding identifiers without
subpatterns and empty tuple patterns are treated as syntactically irrefutable.
These are source counters, not certified runtime complexity: identifier patterns
may resolve to constants, guards may disable futures, macro expansion adds code,
and this stage does not resolve names/types. They must not be projected as CRAP.

Unknown macro paths, unresolved aliases, `task_local!`, `macro_rules!` and other
uncertified grammars fail in production source. Existing bounded expression-list,
JSON and vector grammars remain available. This source parser does not resolve
macro shadowing, expand procedural attributes or establish the executed cfg set.
Even successful syntax inventory is not a complete production-code certificate.
Explicit test modules retain the legacy exclusion behavior; this also does not
certify arbitrary test attributes or conditional compilation.

## Validation and retained replay

The Rust tests cover expression/closure ownership, source spans, select guards
and patterns, malformed syntax, unknown/generated macros and legacy behavior.
The Python suite invokes these tests and checks the independent CLI identity.
Local validation passed 5 parser tests, 381 Python tests and 392 product nextest
tests, plus fmt/Clippy, documentation consistency and strict OpenSpec validation.
Project `config check` and `verify --profile ci --all` returned E1000 because
this repository has no `.harness-gate/flow.toml`; no profile was invented.
Hosted CI and delivery acceptance remain pending.

[Retained replay](rust-native-inventory-replay.json) records all 47 files from
Arc-Admin `e5a1ee5f7ec6dbae461355106d469384581d4607`, each source hash, analyzer
binary hash and both modes' real exit codes. It is a syntax replay of retained
source, not a new test/LLVM run or a synthetic replacement for native evidence.
The legacy mode still fails four files; native mode fails thirteen. Native mode
resolves the tracing/select grammar errors in `error.rs` and `main.rs`, but those
files remain blocked because it also rejects unknown expression-shaped macros such
as metrics, anyhow and try_join. `permissions.rs` and `telemetry.rs` still have
unsupported macros. A larger failed-file count is not a coverage regression or
a complete parser claim; it exposes unresolved grammar/provenance work for GH-220.

Coverage thresholds, CRAP thresholds, selection identity, baselines and formal
workflow authority remain unchanged. GH-215 cannot be accepted on this inventory.
