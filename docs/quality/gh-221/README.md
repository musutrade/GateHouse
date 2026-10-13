# GH-221: executable-source classification

F0–F5 implementation is submitted for controller review; acceptance and CI are
pending. The related [OpenSpec change](../../../openspec/changes/rust-native-missing-file-classification/proposal.md)
extends the accepted [GH-220 production mapping](../gh-220/production.md).
ADR-0040, ADR-0044 and ADR-0049 remain unchanged.

All **47** backend source hashes agree across the committed Arc-Admin archive,
GH-219 replay, GH-220 baseline/final replay, retained production sources and actual
compiler SourceMaps. The new file report accounts for 42 measured files and five
files with no runtime origins in the **declared MIR compilation**. No source was
removed. The classifier has no filename exclusion list.

| Source under `backend/` | Compiler evidence in the selected production unit | Classification |
| --- | --- | --- |
| `src/handlers/mod.rs` | 7 module declarations; child runtime owners map to child sources | `not_applicable`, declarations only |
| `src/repositories/mod.rs` | 10 module declarations; child runtime owners map to child sources | `not_applicable`, declarations only |
| `src/services/mod.rs` | 9 module declarations; child runtime owners map to child sources | `not_applicable`, declarations only |
| `src/permissions.rs` | 33 declarations and 10 CTFE definitions; expansion of `define_permission!` yields unit structs, const constructors, impl declarations and associated `CODE` constants | `not_applicable`, compile-time definitions only |
| `src/permissions/departments.rs` | 7 declarations and 4 CTFE definitions: two string constants and two associated `CODE` constants | `not_applicable`, compile-time definitions only |

Module export declarations in `lib.rs` and `permissions.rs` prove the selected
compiler loaded these files. Exporting a module does not create machine code in
its forwarding file. The permission macro's absence of runtime owners is proved
by the complete post-analysis definition/CTFE inventory and LLVM ownership join,
not by macro syntax. `RequirePermission` runtime implementations and their real
LLVM instances belong to `auth.rs`; the five-file provenance retains those
consumers separately. A macro producing functions, derives or `task_local!`
runtime initialization does receive measured owners in the real fixture.

The full configuration is retained in `backend-classification.json.gz.identity`:
source commit `e5a1ee5f7ec6dbae461355106d469384581d4607`, rustc 1.97.1 commit
`8bab26f4f68e0e26f0bb7960be334d5b520ea452`, LLVM 22.1.6,
`x86_64-unknown-linux-gnu`, selected Cargo production lib/bin targets, no selected
package features, `opt-level=0`, `mir-opt-level=0`, coverage instrumentation and
dead-code linking. Samples are `openapi_contract` and
`permission_template_contract`; no database was accessed. Every row binds the
same inventory/configuration/manifest identity and preserves independent owner
counts, symbols and source origins. `owner_regions` is the sum of referenced
owner block counts, **not** per-file line coverage or an additive global total.
Non-applicable rows have null metrics, never fabricated 0/0.

Coverage remains **725/8025 lines**, **7680/46315 MIR regions** and **497/1778
functions**. The report preserves `measurement_passed=false` and
`baseline_accepted=false`; line/region >=80% and CRAP <=30 remain unchanged.
Complete classification does not mean coverage passes. GH-215 remains blocked
from automatic enablement, and no baseline or debt was accepted.

## Evidence and limitations

- `backend-classification.json.gz`: all 47 detailed rows with complete definition,
  expansion, owner, LLVM symbol and counter provenance.
- `file-summary.json`: all 47 hashes, scoped classifications and owner totals.
- `five-file-provenance.json.gz`: the five complete source texts, definitions and
  expansion chains, exported child owner links, `lib.rs` module exports and
  `auth.rs` permission consumers. Reporting these five historical names does not
  influence the generic classifier.
- `source-reconciliation.json`: per-file hashes from all six source/evidence
  views, compiler unit/SourceMap IDs and the precise archive integrity result.
- `native-fixtures.tar.xz`: new independent default/extra-feature compilations,
  raw profiles, binaries, LLVM exports, inventories, reports and reviewed anchors.
- `artifact-index.json`, `validation.json`, `validation-logs.tar.xz`: artifact
  hashes, exact local commands, exit codes and complete check logs.

The accepted GH-220 raw anchor is
`e4481cad305bd0be28bcdfc37f20f018b3927d5a40d36d0f5c80d49594f1d133`;
the separately reviewed compressed report SHA is
`8464f732540ac476ee20cada1afa4cd36c8a16fa80fb741991c7239a12612a2b`.
This run verifies the committed archives, every retained raw artifact, Cargo
selection/configuration and the complete compiler-to-LLVM mapping against that
report. Seven original backend binaries (`binaries/0` through `binaries/6`) were
not committed. Therefore this is **reviewed archive replay**, not a fresh backend
re-export/certification. No historical external binary/tool path was opened. New
fixture captures have independent anchors and do not replace old backend binaries.

The committed source archive is truncated: its indexed SHA matches, but
`gzip -t docs/quality/gh-220/arc-admin-e5a1ee5.tar.gz` exits 1 with
`unexpected end of file`. All 47 backend source members nevertheless exist and
match the independent source and compiler hashes. We authenticate those backend
inputs, not the entire archive; the original archive is preserved unchanged.

The issue's original 47-source/42-record observation belongs to
`llvm-file-summary-unfiltered/1`. Its original configuration and export are not
authenticated here. Its five missing rows remain **measurement_error** with null
metrics in the historical section. The numerically identical new MIR count is
not evidence of equivalence and does not repair that history. None of these
absence conclusions applies to another feature, target or test selection.

## Reproduction

From this workspace, inspect committed evidence without rebuilding the backend:

```sh
python3 docs/quality/gh-221/reproduce.py --output target/gh-221-replay
```

The script verifies committed input hashes, extracts the retained raw archive to
the requested local directory, authenticates the accepted report/anchor, remaps
all owners and compares every source hash. It records the source archive error
while requiring all 47 backend sources to reconcile. It never uses old external
paths. Its generated compressed report is deterministic for this classifier.

For new native positives, follow the driver's pinned bootstrap instructions and
run the full quality tests with `NATIVE_DRIVER` and `NATIVE_DRIVER_SYSROOT` set;
see `validation.json` for this run's exact environment and commands. The fixture
compiles identical sources with default and `extra` features. Both use the
`contract` sample. A compatible feature witness proves that `conditional.rs` is
outside the default selection; without it the file remains `measurement_error`.
`orphan.rs` remains an error in both configurations. Neither fixture classification
claims complete inventory coverage. Actual zero counters for `unexecuted.rs`
retain a positive denominator. Mutated exports, owners, definitions, CTFE, sources,
manifests and incompatible series/configurations are rejection tests only.

There is no project-local `.harness-gate/flow.toml` with a `ci` profile in this
checkout. `harness-gate config check` and `harness-gate verify --profile ci --all`
are not applicable, not passed. Lifecycle validation uses the tool's real
nextest acceptance receipts instead. No generic project configuration was added.

## Local validation

- `cargo nextest run --manifest-path tools/harness-gate/Cargo.toml --locked`:
  392 passed, zero skipped; 12 real lifecycle acceptance cases retained.
- Harness Gate and native driver `cargo fmt -- --check` and
  `cargo clippy --all-targets -- -D warnings`: passed. Native driver `cargo test`
  also passed (zero Rust unit tests; its native behavior is exercised by Python).
- `python3 -m unittest discover -s tools/quality/tests -v`: all 407 passed,
  including eight new native classification tests with real default/extra builds.
  The first full run found the missing Python boundary inventory row; the row was
  added and the full suite rerun successfully. Both logs are retained.
- Strict OpenSpec validation: all 57 items passed. Complete `critical_paths`
  inventory validation and docs consistency passed.

The first docs check inherited a host Cargo target path and failed with
`/home/gem/cargo-target/debug/.cargo-build-lock: Read-only file system (os error 30)`.
The successful rerun sets `CARGO_TARGET_DIR` inside this workspace. The failed
attempt, exact error and corrected environment are retained; no external cache
was modified. `native-bootstrap.json` records the pinned compiler-development
component hash, bootstrap command and successful exit. Fresh fixture manifests
verify all 74 default and 75 extra-feature sealed artifacts.
