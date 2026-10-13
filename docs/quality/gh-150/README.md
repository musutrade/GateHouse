# GH-150: retained Rust differential compatibility acceptance

This slice implements OpenSpec `consolidate-generic-quality-core-into-rust`
tasks 5.1–5.4. GH-149 was complete before implementation: PR #156 merged as
`4851e3b74605e286bf5e615ccf25be22afe5de57`, and Required Quality Aggregate and
applicable required checks passed on `1971d89b75c4e3b59e5a2afd054e07954970465e`.

The candidate library exposes `replay::evaluate`, `compare`, and `replay`, plus a
migration-only `differential_replay` Cargo example. This example is not installed
with the released CLI. Its JSON transport requires a version, nonempty uniquely
named cases, policy, head project/evidence/expected context, source/artifact roots,
a fixed clock and expected output. Base context, selection, mappings and exceptions
remain explicit optional inputs; omitted values are not inferred from records.
Malformed transport exits 2; semantic mismatches produce an artifact and exit 1;
complete equivalence exits 0. Every result declares `authoritative: false`.

Run from the repository root (use a writable Cargo cache and target directory):

```bash
cargo build --manifest-path tools/harness-gate/Cargo.toml --locked -p harness-gate-quality-core --example differential_replay
python3 tools/quality/fixtures/generic-core/differential.py --rust-replay tools/harness-gate/target/debug/examples/differential_replay --output target/quality/generic-core-differential.json
```

If `CARGO_TARGET_DIR` is set, use its `debug/examples/differential_replay` path.
The Python driver verifies the frozen manifest, external schemas and blob hashes,
checks the current Python reference against every complete frozen output, and
materializes retained bytes for Rust. It never updates goldens or invokes native
collectors, Cargo, Node, coverage tools or Angular builds. To inspect or run the
transport independently, use `--prepare WORK --output INPUT.json`, then invoke
`differential_replay INPUT.json OUTPUT.json`. Keep WORK until evaluation finishes.

The comparator retains every differing field at an escaped JSON Pointer, expected
and actual values, and explicit presence flags. Array order, absent versus null,
integer precision, exact ratios, identifiers, blockers, requiredness, debt/trend,
indexes and evidence links remain significant. The only diagnostic classification
is the already accepted GH-147/GH-149 missing-file wording difference: both reasons
must identify the same logical source/artifact path and missing-file error. These
variations remain visible with both original strings; no unexplained difference
is allowlisted. The replay test also injects a changed aggregate to prove detection.

## Acceptance coverage

| Surface | Retained acceptance |
| --- | --- |
| Rust | Both GH-96/GH-97 production and base/head risk projections; six positive cases and eight negative mutations |
| TypeScript/Angular | GH-134 compatible and regression threshold/debt pairs; exact source identities and coverage numerator/denominator values; sixteen cases including negatives and empty selection |
| Contracts | Compatible and breaking GH-133 contracts, plus unknown producer; green local gates cannot hide a project blocker |
| Full machine outputs | Evidence validation, complete policy results and complete project reports compared to frozen Python outputs |
| Missing/stale/tampered evidence | Rust and Angular missing records, caller-context mismatch, modified artifacts and missing sources |
| Identity and capability | Duplicate subjects, unknown capability, required unsupported state, empty explicit selection |
| Values, base and exceptions | Malformed typed measurements, incompatible series, absent base, invalid exception metadata |
| Provenance and contracts | Frozen unknown producer and breaking contract, plus existing Rust/Python differential report tests for missing/mismatched bindings, stale baseline, missing consumer/client evidence, drift and tampering |

`retained_corpora_and_consolidated_negative_matrix` runs in the required nextest
suite. It checks complete output equivalence and independently asserts that each
of twelve negative categories is represented and fails closed. The existing
399 evidence/project comparisons and 826 policy/report comparisons extend the
frozen matrix with boundary mutations. Python tests retain native-adapter parsing
negative coverage without collecting new evidence.

All 329 Rust tests and 289 Python tests passed. Standalone replay matched all
33 cases with zero unexplained mismatches and 227 retained missing-file wording
variations. Formatting, Clippy, documentation consistency and strict OpenSpec
validation passed. The executable also returned exit 1 with the exact field path
for an injected favorable outcome and exit 2 for an empty transport.

The [acceptance summary](https://github.com/musutrade/Harness-Gate/blob/d9a932b23dcfea709175285b594ca200450a9971/docs/quality/gh-150/acceptance-summary.json) records every case outcome and
the exact retained Angular source identities and coverage counters.
Validation results and exact commands are recorded in [validation.json](https://github.com/musutrade/Harness-Gate/blob/d9a932b23dcfea709175285b594ca200450a9971/docs/quality/gh-150/validation.json),
with complete logs and differential field evidence in [validation.tar.gz](https://github.com/musutrade/Harness-Gate/blob/d9a932b23dcfea709175285b594ca200450a9971/docs/quality/gh-150/validation.tar.gz).
The checkout has no `.harness-gate/flow.toml` declaring a `ci` profile, so
`harness-gate config check` and `harness-gate verify --profile ci --all` are not
applicable, not passed. The initial shared Cargo cache failure is retained and
resolved with workspace-local cache/build directories.

This acceptance is bounded to the retained runs and accepted measurement series;
it certifies no new ecosystem. Existing required CI names, dependencies, thresholds,
collector selection and release authority remain unchanged. Tasks 6–7, hosted
shadow integration and authority transfer remain outstanding under
[ADR-0040](../../adr/0040-language-agnostic-evidence-policy.md). Required hosted CI
for the final submitted SHA belongs to the controller; the full proposal is not
accepted by this slice.
