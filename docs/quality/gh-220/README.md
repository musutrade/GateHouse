# GH-220: historical prototype checkpoint

The following is the retained initial checkpoint. The subsequent compiler implementation
and full backend results are in [production.md](production.md); original failure
records below remain unchanged. At this checkpoint GH-220 was not complete.
This historical evidence does not release GH-221 or complete
GH-215. No production collector, threshold, accepted baseline or Arc-Admin test
was changed. The new adapter is an opt-in experiment, not an authoritative gate.

## Reproduction and source identity

PR #222 merged as `bfceab076ad3a1760c75ab412337ddc93959ec4d`; `pr-222.json`
records the merged PR and successful required checks. This supports closing the
previous proposal's M4.2 only.

Arc-Admin was fetched from GitHub into this workspace's `target/gh-220/arc-admin`
at `e5a1ee5f7ec6dbae461355106d469384581d4607`. All 47 `backend/src/**/*.rs`
hashes match the committed GH-219 replay. The original analyzer was also rebuilt
from this repository's merge commit in `target/gh-220/baseline-source`; its replay
reproduces every original exit status (legacy: 4 failures; native: 13 failures).
`source-replay-before.json` retains the first reproduction;
`source-replay-baseline.json` retains the independently rebuilt reproduction.
`source-replay-after.json` preserves an intermediate attempt, and
`source-replay-final.json` records the final syntax result: legacy 4, native 3.
All are **syntax evidence, never coverage evidence**.

```sh
git clone --no-checkout https://github.com/musutrade/arc-admin.git target/gh-220/arc-admin
git -C target/gh-220/arc-admin checkout --detach e5a1ee5f7ec6dbae461355106d469384581d4607
CARGO_TARGET_DIR="$PWD/target/gh-220/analyzer" cargo build \
  --manifest-path tools/quality/rust-measure/Cargo.toml --locked
python3 tools/quality/rust_native_replay.py --source target/gh-220/arc-admin \
  --analyzer target/gh-220/analyzer/debug/harness-gate-rust-measure \
  --output target/gh-220/new-replay.json
```

The native syntax identity is `source-decisions-native-2` / v0.2.0. Explicit
metrics, anyhow/bail and tokio::try_join grammars visit nested expressions and
closures; macro tokens and visited attribute syntax retain source spans.
Bare-name grammars do not resolve import aliases or shadowing. Compiler provenance
remains unresolved. `sqlx::migrate!`, `macro_rules!`/generated permission invocations
and `tokio::task_local!` still reject their containing files. The old analyzer and
`mccabe-rust-3` identity are unchanged.

## Real native fixture

The fixture runs with rustc 1.97.1 (commit
`8bab26f4f68e0e26f0bb7960be334d5b520ea452`), LLVM 22.1.6 and
`RUSTC_BOOTSTRAP=1`. Retained evidence includes compiler/tool identities, source,
cfg, expansion hygiene, InstrumentCoverage and aggregate MIR, LLVM IR, binary,
raw profile, merged profile, export, demangling, commands and output streams.
The artifact index identifies the compressed bundle and manifest anchors.

The bounded join finds 8 owners and 9 LLVM instances, including two generic
instances. It observes the async constructor count of 1 and its unpolled body
count of 0 independently. Macro instances retain distinct hygiene contexts even
where definition spans coincide. Raw results are lines **17/19**, regions
**28/36**, functions **7/8**. The closure and async body fail unchanged thresholds;
this is a successful bounded measurement with a failing quality result.
Enabling `feature="extra"` yields 9 owners and a different comparison identity.
The cfg(test) module is absent from this normal binary's compiler inventory.

The experiment verifies a trusted manifest, cross-checks aggregate MIR, and
re-exports the retained binary/raw profile with hash-matched local LLVM tools.
It does not execute archived programs. Native-positive tests compile Rust;
mutated JSON is used only for negatives. The report's bounded LLVM-join flag does
**not** certify complete invocation-span provenance: `source_provenance_complete`
and `backend_complete` are both false. The independent experimental series has
exact rational CRAP and compatibility checks, but no production debt/delta adapter.

## Actual backend evidence and blockers

`cargo test --locked --no-run` compiled the pinned backend with coverage enabled.
The existing OpenAPI and permission-template contract binaries then ran **2 + 5
tests successfully**, without a database. Their native export contains 136,920
functions across 3,182 files, including dependencies and tests; it is retained
**unfiltered diagnostic evidence**, not production metrics or the old reference
series. Database/API-flow tests were not run; full backend sampling is unproven.

The production-library InstrumentCoverage audit observes 1,723 function MIR bodies,
961 without code regions, and 30 groups of ambiguous display names. This count is
not a complete backend inventory: other targets and generated constructors may
bypass this MIR pass. The real `#[derive(Clone)]` fixture shows both an
uninstrumented clone and constructors absent from that pass; certification rejects
the aggregate-MIR mismatch. rustc explicitly skips automatically-derived coverage
in its [coverage query](https://doc.rust-lang.org/stable/nightly-rustc/src/rustc_mir_transform/coverage/query.rs.html).
See also [native coverage](https://doc.rust-lang.org/rustc/instrument-coverage.html)
and [MIR dumps](https://rustc-dev-guide.rust-lang.org/mir/debugging.html).

Initial expansion/MIR builds omitted a profile destination for compiler-loaded
instrumented dependencies. Their logs contain `LLVM Profile Error: Failed to
write file "default_...profraw": Read-only file system`. Retries with an explicit
workspace profile destination replayed these cached Cargo dependency diagnostics;
the MIR retry emitted no new dump directory, so it is not counted as new evidence.
Original logs are retained. No successful external profile writes or database
access are claimed. A failed fixture invocation with an incorrect analyzer path
and the initial regression failure are retained too.

Remaining acceptance work is concrete: compiler definition IDs and full expansion
invocation edges; complete generated-code observation; all Cargo target/dependency
owners; independently verifiable backend test exclusion; a complete production
series integrated with historical debt/delta rules; and complete backend evidence.
Existing raw outputs demonstrate these gaps rather than waive them.

## Validation and retention

`validation.json` records exact commands, exits and limitations; archived logs
retain successes and failures. No `.harness-gate/flow.toml` exists here, so
`harness-gate config check` and `harness-gate verify --profile ci --all` are **not
applicable**, not passed. The complete critical_paths inventory is validated
without modifying any source hashes or baseline.

The index distinguishes committed compressed evidence from workspace-retained
large backend binaries. Raw originals and failed attempts remain available in
this task's workspace; no historical external workspace was used. Replaying an
archive requires the pinned toolchain and the manifest SHA from the trusted index,
not a replacement anchor supplied alongside edited raw files.

Related: [proposal](../../../openspec/changes/rust-native-production-mapping/proposal.md),
[design](../../../openspec/changes/rust-native-production-mapping/design.md),
[tasks](../../../openspec/changes/rust-native-production-mapping/tasks.md),
ADR-0040, ADR-0044 and ADR-0049. The PR must remain explicitly incomplete; no
`.symphony-handoff.json` completion declaration is authorized by this evidence.

### Replay retained native evidence

Verify the archive digest against `artifact-index.json` before extraction. Extract
`native-fixtures.tar.gz` into a fresh directory inside this workspace. With the
pinned Rust/LLVM tools installed, certify its `complete/` directory using the
`evidence_sha256` recorded in `native-fixtures.json`:

```sh
mkdir -p target/gh-220/archive-replay
tar -xzf docs/quality/gh-220/native-fixtures.tar.gz -C target/gh-220/archive-replay
python3 tools/quality/rust_native.py certify \
  --evidence target/gh-220/archive-replay/complete \
  --expected-sha256 8ddb8b0fd9f5bd5c402e0754ec0846a54b6e5da8c13876462d1b28bb4479e683 \
  --output target/gh-220/archive-replay/report.json
```

Expected exit is **1**, with the recorded real threshold failures. The `derived/`
case rejects `MIR inventory omission/ambiguity`. Backend raw text, profiles, MIR,
LLVM export and original logs are in `backend-diagnostics.tar.xz`; its two large
sample binaries are retained only at paths and hashes in `backend-binaries.json`.
Re-exporting backend profiles needs those exact binaries. The committed bundle
alone therefore cannot reproduce the backend native export.

The first nextest run failed because the workspace-local temporary directory let
its no-Git fixture discover the parent repository. The retry sets
`GIT_CEILING_DIRECTORIES=$PWD/target/gh-220/tmp` while keeping `TMPDIR` there.
The first Python run found missing architecture-inventory entries for these two
new modules; those entries were added before rerunning. Original failures remain
in the validation archive. No test or threshold was relaxed to address either.

The last documentation-check retry passed with an explicit workspace-local
`CARGO_TARGET_DIR`. The preceding invocation omitted this setting and Cargo could
not open its global build lock (`Read-only file system (os error 30)`); its failed
report and diagnostic command are retained. Final local results are 392 nextest
tests, 390 Python tests, analyzer tests/fmt/Clippy, harness fmt/Clippy, workflow
acceptance, strict OpenSpec validation and the complete critical_paths check.
Passing these checks does not close the production mapping gaps above.
