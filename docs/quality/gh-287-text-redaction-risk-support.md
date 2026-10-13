# GH-287 text redaction risk measurement support

This change extends the required repository risk boundary only to
`tools/harness-gate/src/utils/redaction.rs`, which is already in
`production-source.json`. Adding a path is not evidence of certification: the
source inventory, instrumentation and native ownership checks below must pass.
The selection identity is `gh287-text-redaction/1`. The analyzer remains
`harness-gate-rust-measure/0.3.1`, with `mccabe-rust-3/1`,
`closure-black-box/1`, `insertions-utf8/1` and
`compiler-target-production/1` unchanged. No parser or dependency change is
made to the analyzer.

The certified source shape includes static `OnceLock` initialization,
`get_or_init` with an initializer closure, `vec!` expressions containing regex
and replacement tuples, and `iter().fold` with a separate closure owner. Regex
pattern contents are string data, not Rust decisions. The production inventory
must contain exactly `redact_text` and these two closures, each with Rust CC 1;
the inline test module is excluded. The actual checkout source is independently
parsed, instrumented and reparsed, with original UTF-8 byte spans and body ranges
recovered after insertion. No parent execution count substitutes for a closure.

`SourceMeasureTests.test_once_lock_regex_tuple_fold_has_independent_native_owners`
builds a separate temporary Cargo executable with actual `regex` patterns and
tuples. Its regex version and transitive dependency versions, registry sources
and checksums come from the repository's current `tools/harness-gate/Cargo.lock`.
The fixture retains this original lock and its exact subset lock, explicitly
fetches with `cargo fetch --locked` for the configured target, then builds with
`--locked --offline`. Both commands are logged, and the exact lock bytes are
checked after each command. Fresh native runners do not need a preexisting
regex cache; dependency resolution cannot change the pinned lock. Its target
directory is private to the fixture; no shared prebuilt rlib is supplied.

Each mode executes the wrapper once in a separate process/profile:

| Mode | Initializer count | Fold closure count |
| --- | ---: | ---: |
| `fresh-zero` | 1 | 0 |
| `fresh-multi` | 1 | 2 |
| `preset-zero` | 0 | 0 |
| `preset-multi` | 0 | 2 |

Preset modes use `OnceLock::set` before calling the wrapper. Every mode must
retain three distinct native owner records. Both line and region denominators
must remain nonzero even for an unexecuted closure; its covered counts must be
zero despite the parent's execution. Executed owners must cover their complete
original line and region denominators. Each mode separately rejects deletion
of the parent, initializer and fold record, including zero-hit records. Both
closures reject substitution of the parent's region mapping with a specific
function-kind mismatch error. These assertions use real LLVM exports; negative
copies never replace the original export.

The following existing checks are reused rather than duplicated:

- `test_instrumentation_distinguishes_unexecuted_closure_and_rejects_missing_evidence`:
  altered original digest, changed instrumented bytes, unsupported LLVM macro
  expansion, missing active function/closure and inactive body exclusion.
- `test_compare_rejects_mixed_target_configurations`: changed or missing compiler
  configuration.
- `test_ratchet_preserves_uncovered_unchanged_closure_debt`: mixed measurement
  series and unchanged closure debt, plus changed selected function failure.
- `test_unsupported_configuration_fails_without_partial_inventory` and
  `test_unknown_macro_fails_closed`: unsupported syntax remains blocking.

The added `test_actual_redaction_source_inventory_and_instrumentation` checks
the exact current production callable set and byte-range reproduction.
`RiskScopeTests.test_process_test_module_reaches_measurement_but_unknown_sources_block`
now checks that redaction reaches collection while unknown utils sources are
rejected before the analyzer build. Existing full-manifest, tool-identity and
source/commit checks remain authoritative; no production-change exemption is
introduced.

Focused verification, performed by the designated validation agent:

```sh
python3 -B -m unittest discover -s tools/quality/tests -p test_source_measure.py -v
python3 -B -m unittest discover -s tools/quality/tests -p test_ci_quality.py -v
```

Set `RUST_MEASURE_NATIVE_EVIDENCE` to a new evidence root. The existing native
configuration fixture writes to its `cfg-closure` child directory; the new
OnceLock fixture uses a unique `once-lock-fold-*` child. Without this variable,
the latter retains evidence under `target/gh287-native-evidence`. It retains
the original/instrumented source, insertion manifest, Cargo inputs/locks,
executable, raw profiles, merged profiles, LLVM exports, command arguments,
explicit compilation/profile environment, stdout/stderr and exit codes. Its
results preserve callable names, kinds, source/syntax hashes, source ranges,
native instance identities/counts and complete line/region denominators. The
success marker `native-result.json` is written only after all mode and negative
checks pass, with analyzer/compiler/tool/test/input hashes and target cfg.
Partial logs remain on failure. This is native execution on the actual host;
cross-target AST parsing does not certify another host's LLVM mapping.

After the fixtures pass, the final measurement tool and selection must collect
both base `f3c6d62d511abff1044c69c42ae7c50a26416bfb` and the exact final head
commit through the existing snapshot, AST, instrumentation, native coverage,
measurement and comparison stages. Both inventories must independently contain
the actual three redaction owners; original bytes must match each commit and
all tools and compiler configuration must agree. Preserve the manifests, raw
LLVM evidence, reports, comparison and failed attempts. Historical reports
cannot be relabeled or reused as evidence for the extended selection.

This support change leaves Engineering Policy semantics, CRAP and coverage
formulas, requiredness, thresholds, debt, ratchet and baseline rules unchanged.
Only this source boundary is extended. `process/command.rs`, `process/task.rs`,
`process/capture.rs` and `service/lease.rs` remain outside the required risk
support boundary and require separate certification before production changes.
The design and configured tests alone are not a successful certification or a
release PASS; record the actual checkout SHA, host, target, tool hashes, test
outcomes and base/head comparison in the validation handoff.
