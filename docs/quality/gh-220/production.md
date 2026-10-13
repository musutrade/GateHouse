# GH-220 native production mapping

The compiler-integrated production mapping now certifies the pinned Arc-Admin
backend for the declared build configuration. Its genuine measurements fail the
unchanged quality thresholds. Acceptance and CI remain controller-owned; this
submission does not release GH-221 before merge/acceptance or complete GH-215.

The implementation is in [the pinned driver](https://github.com/musutrade/Harness-Gate/blob/8d93ea53c7cf8b314bda816eafe13209da25c742/tools/quality/rust-native-driver/README.md),
[collector](https://github.com/musutrade/Harness-Gate/blob/8d93ea53c7cf8b314bda816eafe13209da25c742/tools/quality/rust_native_driver.py) and
[typed policy projection](https://github.com/musutrade/Harness-Gate/blob/8d93ea53c7cf8b314bda816eafe13209da25c742/tools/quality/rust_native_policy.py).
Generic Core, Arc-Admin application tests, formal gates and baselines are unchanged.

## Actual backend scope and result

Source commit: `e5a1ee5f7ec6dbae461355106d469384581d4607`. All 47 backend
source hashes match the committed GH-219 replay and the retained compiler sources.
The capture compiled the library and all five binaries with default Cargo features,
plus the existing `openapi_contract` and `permission_template_contract` integration
tests. Those tests passed 2 + 5 cases without a database. No API/frontend/full-stack
or database-backed workflow acceptance is implied by this sample.

| Production metric | Raw result | Meaning |
| --- | --- | --- |
| Functions | 497 / 1,778 | Independent owner entry counters |
| Lines | 725 / 8,025 | Union of all local MIR source origins |
| Regions | 7,680 / 46,315 | Independent MIR basic blocks |

These are `rust-native-production-mir-block/1` facts, never the old
`llvm-file-summary-unfiltered/1` series. Regions and complexity are explicitly
versioned in the [design](../../../openspec/changes/rust-native-production-mapping/design.md).
Every per-function report retains exact rational CRAP, raw block counts, LLVM
instance symbols, and one-to-many source lines. Compiler definitions additionally
retain generated types and constant-evaluation MIR, including permission macros.
The [expansion audit](https://github.com/musutrade/Harness-Gate/blob/d9a932b23dcfea709175285b594ca200450a9971/docs/quality/gh-220/production-expansions.json) indexes actual tracing/select,
task_local, macro_rules, metrics, anyhow, try_join and derive provenance.

The complete selected backend mapping is verified; no missing owner is waived.
Dependencies/std/generated dependency output and integration-test owners are
explicitly excluded and remain in raw inventories/LLVM output. Source files absent
under a selected cfg remain in the lexical inventory. This configuration does not
certify other features/platforms, mixed cfg(test) production crates, uninstantiated
generics, or arbitrary generated project paths; unsupported cases fail closed.

## Reproduction and evidence

Follow the driver's README to bootstrap the matching rustc-dev component into a
fresh workspace-local overlay and collect a new capture. The bootstrap checks the
official archive SHA and compiler commit; its successful local replay is retained.
All build targets and profiles stay inside this task workspace. Incremental MIR
caches are disabled: the earlier compiler panic and raw failure remain preserved.

[production-artifacts.json](https://github.com/musutrade/Harness-Gate/blob/d9a932b23dcfea709175285b594ca200450a9971/docs/quality/gh-220/production-artifacts.json) is the artifact inventory:
full backend mapping/report and raw compiler/profile/LLVM bundle, exact retained
binary paths/hashes, source and tool identities, native fixture evidence and
validation. Large backend binaries remain in this workspace rather than Git;
**the committed raw bundle alone cannot re-export LLVM without those exact indexed
binaries and hash-matched local tools**. It supports source/MIR/LLVM inspection.
The fixture archive includes its actual binaries and profiles. Evidence anchors
must be reviewed separately; resealing changed evidence does not authenticate it.
The verifier re-exports counters, and never executes an archived binary.

The final backend evidence anchor is
`e4481cad305bd0be28bcdfc37f20f018b3927d5a40d36d0f5c80d49594f1d133`.
Certification returns exit 1 with a complete report because thresholds fail.
Historical [prototype evidence](README.md), syntax failures and earlier backend
failures remain available; they are not relabeled as successful production proof.

## Policy, validation and limits

Both compatible base and head captures require independently supplied anchors.
Tool bytes, compiler/config/target selection, Cargo inputs, mapping rules,
collector/projection hashes and hotspots partition the series; missing or
incompatible history rejects. The released Rust evaluator preserves unchanged
legacy debt and rejects changed debt or new regression. Tests use real compiled
captures and the actual Rust command, including CRAP 7 → 56 failure. No Arc-Admin
baseline is accepted or synthesized for the first production measurement.

[production-validation.json](https://github.com/musutrade/Harness-Gate/blob/d9a932b23dcfea709175285b594ca200450a9971/docs/quality/gh-220/production-validation.json) records exact final local
commands, exit codes and logs. Native positive regressions compile/sample/export
real Rust; mutated JSON only exercises negative rejection. Earlier failed attempts
are preserved. Project-local `config check` and `verify --profile ci --all` are not
applicable because this repository has no `.harness-gate/flow.toml`/`ci` profile.

GH-215 still owns cross-repository integration, immutable baseline acceptance,
frontend/API/full-workflow assurance and gate migration. GH-221 is not started;
it may use this inventory only after GH-220 controller acceptance and merge.
