# Core CI plugin-test handoff

Engineering Policy semantics are unchanged. Rust Core retains decisions,
requiredness, thresholds, measurement identity, baseline, debt/ratchet and release
authority. Frozen Python policy/reference modules remain non-authoritative.

## Accepted execution

GateProbes [PR #18](https://github.com/musutrade/GateProbes/pull/18) preserves
original collector tests and imports in `compatibility/legacy-core/snapshot`.
The accepted candidate is `a762f0c4c7da3945367b870bccc39aac8069132f`;
[CI 38406597322](https://github.com/musutrade/GateProbes/actions/runs/38406597322)
succeeded. Its required compatibility job ran **103 tests, zero failures/errors/
skips**, proved Core fixture parity against the original Angular/V8 producer,
and reported eleven held bundled-runtime methods separately.
[The original result](issue-10/legacy-compatibility.json) lists every inherited
method, owner, scope and execution/hold reason. GateProbes' frozen
[inventory](https://github.com/musutrade/GateProbes/blob/a762f0c4c7da3945367b870bccc39aac8069132f/compatibility/legacy-core/inventory.json)
binds each imported test/helper/fixture to its original bytes and owner.

The same run accepted all five current plugins against released GateHouse Core
0.4.9, including native Linux, Windows, macOS Intel and macOS ARM, and tested the
external preset loader. Linux-only classification remains Linux-only. The
original native four-module parity run
[38342999155](https://github.com/musutrade/GateProbes/actions/runs/38342999155)
is retained in the receipt; its scopes and source digests are unchanged.

## Ownership and scope

| Modules / methods | Owner and execution | Original boundary |
| --- | --- | --- |
| `test_rust_collector_project`, `test_rust_native_driver`, `test_native_artifact_paths` | GateProbes native tests, original four-platform matrix | Existing project/driver/path parity; adapted artifact-root location only |
| `test_rust_native_classify` | GateProbes native Linux | Compiler owner classification; no Windows/macOS widening |
| `test_rust_collector_contract`, `test_rust_collector_delivery` | GateProbes compatibility Linux | Legacy tuple, receipt, malformed input, relocation, exact Core identity, typed failures and exit behavior |
| `test_rust_collector_runtime.BuildInputTests`, `.InstalledLayoutTests` | GateProbes compatibility Linux, both execute | Changed/unsafe build inputs and installed host-schema lookup |
| `test_rust_collector_runtime.StandaloneNativeTests` (11 methods) | GateProbes preserved source; explicitly held | Suspended assembled private Linux runtime; originally opt-in via `RUST_COLLECTOR_RUNTIME`, not independent native RC acceptance |
| `test_rust_native` (8 methods) | GateProbes compatibility Linux, exact Rust 1.97.1 / LLVM tools | Original bounded textual MIR experiment and negative replay cases; no stable candidate promotion |
| `test_typescript_reference`, `test_typescript_semantics`, `test_typescript_acceptance` | GateProbes compatibility Linux, including inherited methods | Original Angular/V8 archives, counters, source maps, UTF-16, provenance, contexts, transport and debt replay windows |
| `test_typescript_contracts`, `test_typescript_advisory` | GateProbes historical adapter/reference replay | Original GH-133 contract relationships, provenance and advisory failure/inventory checks; no authority transfer |
| `test_typescript_policy` (9 methods) | Core; original methods also run in GateProbes to prove fixture parity | Generic requiredness, mixed Rust/TypeScript aggregation, series compatibility, debt and lineage |
| `test_preset_packs` (6 methods) | Core; original TypeScript producer method also runs in GateProbes | Generic composition, accepted series/rules, required/unsupported states and frozen provenance |
| Source-risk standalone Rust + six Python modules | GateProbes existing source-risk job | Original full source-risk inventory; no duplicate AST build in Core |

The new TypeScript original-source plugin does not replace or certify the old
Angular/V8 adapter. Paused bundled-runtime tests are preserved with their
original helper imports and are never counted as passing assurance. This change
does not construct, publish or revive the held product. Their assembly/publication
workflow disposition remains tracked separately by GateHouse #12.

## Ordinary Core selection without plugin source

`tools/quality/tests/run_core.py` uses `core-plugin-test-handoff/v2`. The receipt
separates mandatory Core inputs from accepted transferred inputs. A transferred
copy may be absent; any remaining copy must still match its accepted digest.
Transferred trees reject unknown tracked files. Core consumers, evidence/schema
contracts, policy fixture and generic test inputs remain mandatory and guarded.

Core's TypeScript policy and preset tests now read fixed original facts, not a
collector implementation. The fixture retains real source/artifact bytes,
contexts, measurement-series identity and baseline records. Generic validators
still validate those bytes and contexts on every policy evaluation. GateProbes
compares the complete facts, project, raw artifacts and receipts with the original
producer; updating hashes alone cannot establish that parity.

Missing/malformed receipts, wrong owners/commits/runs, failed or skipped
acceptance, incomplete/duplicate method inventories, modified Core consumers or
schemas, and unknown delegations fail. New Core modules run by default; nested
modules need explicit Python package discovery. Only the original four native
module names may use the prior native handoff, and newly transferred modules must
appear with GateProbes ownership in the real compatibility result.

The existing `Quality Script Tests` job verifies the receipt, removes all 24
plugin implementation paths from its disposable checkout and then executes the
ordinary selector. It keeps the same check name and required aggregate. The
manual `core_isolation_check.py` uses this same selector instead of its former
15-module exclusion list. Core-owned preset and TypeScript policy tests therefore
remain in both ordinary and isolated Core execution.

## Cost and rollback

One GateProbes Linux job restores two checksum/member-verified historical data
archives (about 36 MB), executes the original compatibility scope and uploads a
bounded JSON result for seven days. It has a 15-minute timeout. The accepted
result records 44.317 seconds of execution and 62 seconds of hosted job time;
[the hosted cost record](issue-10/hosted-cost.json) retains every selected job; no duplicate complete plugin matrix
runs inside Core. The current Core 0.4.9 five-plugin matrix was required once to
renew cross-repository acceptance, preserving its existing platform selection.
Superseded failed/cancelled runs remain visible in GitHub.

The receipt and original test bodies remain reviewable. If parity is withdrawn,
revert the handoff/selector together and restore local tests before accepting new
Core consumer changes. Historical archives, immutable releases and baseline/debt
inputs remain untouched. Physical source deletion and measurer pin migration are
separate GateHouse #13 and #11 work.
