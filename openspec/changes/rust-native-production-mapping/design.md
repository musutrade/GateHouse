# Design

## Evidence and ownership
Pin source revision and hash every input. Retain compiler identity, target, cfg,
features, compile arguments, expansion provenance, binary, raw profile and LLVM
export. Compiler expansion is required for task_local, macro_rules, derives and
conditional compilation: zero source AST functions does not imply zero code.
Explicit tracing/select/metrics/anyhow/try_join syntax traversal supplies source
provenance only, never a runtime completeness certificate.

Map each LLVM function instance and every region to an observed function, closure,
async body or generated owner. Preserve one-to-many origins and instance identities.
Merge only proven instances; reject duplicate regions, ambiguity and missing
functions. A parent counter cannot supply an unobserved closure's coverage.
Retain excluded test owners and raw regions so production selection is verifiable.

## Independent measurement
Production mapping, selection, complexity and normalization rules form a new
versioned native series with toolchain and build configuration. Never emit
llvm-file-summary-unfiltered/1 for filtered production metrics. Compute line and
region coverage from raw counts and CRAP as exact rationals. Compare only identical
semantic series; preserve historical evidence and baselines unchanged.

## Fail-closed boundaries
Unsupported expansion, cfg, unresolved ownership, tampering and incompatible tools
or series are measurement errors without fabricated metrics. A successful measure
may fail unchanged thresholds. Real minimal compiled fixtures establish only their
bounded mechanism; synthetic mutations test rejection, not native success.

## Rollout
No authoritative collector switch. N0–N6 evidence and remaining gaps are reviewed
in a PR. Controller owns CI, acceptance, merge and issue closure. GH-215's complete
workflow/API/frontend/baseline acceptance and GH-221 remain separate.

## Compiler implementation and trust boundary
`tools/quality/rust-native-driver` pins rustc commit
`8bab26f4f68e0e26f0bb7960be334d5b520ea452` / LLVM 22.1.6 and emits
`rustc-mir-block-inventory/3`. The development driver overrides MIR and coverage
providers to insert an independent physical LLVM counter at every typed MIR basic
block, including constructors and compiler-generated coverage(off) methods.
Each owner has a virtual `.mir-map` source file: LLVM symbol, filename, region
coordinates and block index must agree exactly. Runtime DefPathHash owners and
post-analysis compiler definition IDs must agree; local monomorphized instances
must resolve, all owners must have LLVM instances, and every LLVM region must be
owned or have a unique Cargo dependency/std/test exclusion. Missing instantiations,
ambiguous sources and duplicate LLVM symbols/regions reject the capture.

SourceMap IDs, source bytes/hashes and byte spans retain recursive hygiene edges
with both expansion call sites and definition sites, plus macro definition IDs.
All compiler definitions are retained in index order, including expanded types
and actual constant-evaluation MIR for constants/statics. This proves the role of
`define_permission!` and other non-function macros from compiled definitions,
without inferring absence of executable code from the source AST. The syntax-only
adapter still rejects its three unresolved files; compiler-resolved definitions
provide the separate native production path.

The sealed capture retains all commands, cfg, Cargo metadata/messages, manifests,
lockfile, sources, binaries, raw profiles, compiler inventories and LLVM export.
Certification checks a separately supplied host-reviewed manifest anchor, hashes
local tools and re-exports the retained binaries/profiles; it never executes an
archived program. This proves integrity against the host anchor, not remote
producer authentication. Incremental custom MIR caches are disabled and rejected
because cached SourceMap identities cannot safely be reused.

## Counting and production selection
`rust-native-production-mir-block/1` intentionally uses **MIR basic blocks** as
regions, not stock rustc source regions. Distinct LLVM instances of the same owner
sum their raw counters; distinct owners never share counters. Entry blocks measure
function execution independently, including an unpolled async body's zero count.
Lines union every local statement/terminator source origin, including macro
invocation and definition lines. Any positive owned block covers that projected
line; global lines deduplicate shared origins. Exact CRAP uses these line counts.
Complexity is E−N+2 on the typed normal MIR CFG with a common exit; parallel edges
count, unwind/cleanup, imaginary and coroutine destruction paths do not. Raw CFGs
retain those excluded paths. Nonterminating normal graphs currently reject.

All Cargo lib/bin targets must be compiled. Integration-test targets are separately
inventoried and excluded with their raw counters; mixed cfg(test) production crate
builds reject. Declared dependency roots, generated dependency OUT_DIR roots and
pinned standard-library roots classify foreign LLVM output uniquely. Project
build scripts remain build evidence, outside application metrics. The complete
`src/**/*.rs` lexical inventory retains cfg-inactive files as compiler_loaded=false;
that status means absent under this configuration, never no executable code.
External production paths without declared ownership reject. This issue certifies
the pinned Arc-Admin backend's default features and all six production targets,
with the two existing no-database integration-test binaries as samples. Other
features/platforms and mixed unit-test builds require a new compatible capture.

## Historical policy integration
`rust_native_policy.py` projects certified facts into existing typed generic
contracts and invokes the released Rust `quality evaluate` command. No Generic Core
or Python policy engine changes. Tool bytes, mapping/line/region/complexity rules,
cfg/features/targets, manifest/lock inputs, source selection, adapter/projection
hashes and hotspot selection partition history. Both independently anchored base
and head are required; missing or incompatible history rejects, without accepting
or rewriting a baseline. Existing Rust policy evaluates 80/80/30, changed CC>10,
hotspot, debt and non-regression rules. Unique compiler owner/content identities
support conservative modify mappings; rename/split/merge requires reviewed explicit
mappings and the existing core's validation.

## Evidence and remaining scope
[Production evidence](../../../docs/quality/gh-220/production.md) indexes real
Arc-Admin source, raw artifacts, complete mappings, failed thresholds and actual
validation. Small native fixtures independently exercise omission, duplicate,
ambiguity, tampering, generated/cfg and incompatible-history rejection, including
actual Rust policy debt preservation and regression failure. Historical prototype
failures and bundles remain retained and are not reclassified as successes.
GH-215 baseline acceptance, frontend/API/full-workflow validation and authority
transfer are out of scope; GH-221 remains blocked until controller acceptance.
