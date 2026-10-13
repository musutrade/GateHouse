# GH-288 source-risk toolchain compatibility

The retained Linux x86-64 matrix passed for Rust 1.97.1 / LLVM 22.1.6
and Rust 1.99.0 / LLVM 23.1.1. Each combination passed the existing 26 tests
and seven compatibility tests, including supported capture, independent plugin
re-export and signed Core collection/evaluation. Certification applies to the
retained implementations and supported source shapes below; missing independent
counters and unsupported owners still reject measurement.

## Explicit combinations and prerequisites

| Combination | rustc commit | Matching LLVM tools | Scope |
| --- | --- | --- | --- |
| Rust 1.97.1 | `8bab26f4f68e0e26f0bb7960be334d5b520ea452` | 22.1.6 | Retained source-risk combination; regression control |
| Rust 1.99.0 | `b940084d7eb6a299eb4bfeb8e34901bc051e7ac4` | 23.1.1 | Source-risk matrix passed; bounded certification below |

The fixture supports Linux x86-64. Each selected rustup toolchain must already
have its matching `llvm-tools-preview`. Rust 1.99's tools were installed with
explicit user authorization; the default `stable` toolchain was not changed.
The runner installs no component and never falls back to stable or a PATH LLVM.
It records actual Python, cargo-llvm-cov and compiler/LLVM identities, rather than
inferring compatibility from their names. `cargo llvm-cov` and OpenSSL must be
available. A missing tool, wrong commit/LLVM/host, failed command or timeout is
blocking, with retained original diagnostics.

The [runner](https://github.com/musutrade/Harness-Gate/blob/8d93ea53c7cf8b314bda816eafe13209da25c742/tools/quality/rust-source-risk/test_toolchain_compatibility.py)
sets `RUSTUP_TOOLCHAIN` only in child-process environments and derives actual
rustc, cargo, LLVM paths from that selected compiler's sysroot/host. Two combinations
have separate plugin/source snapshots, analyzer target directories, compiled
inventories, measured targets, profiles, exports and acceptance artifacts. Each
AST build uses the copied committed Cargo lockfile, explicit locked fetch and
offline locked build; lock bytes are checked. No shared compiled analyzer or rlib
from the other combination is trusted. Existing closure/arithmetic/native tests
run against that combination's actual fresh analyzer and LLVM tools.

## Completed matrix and precise source identity

The 2026-10-08 evidence root is
`/mnt/dev-ssd/dev-tmp/gh288-push-20261008/toolchain-matrix/`.
`success.json`, `matrix.json` and `diagnostic-comparison.json` record both
successful combinations; `1.97.1/tools.json` and `1.99.0/tools.json` retain
the compiler/LLVM paths and hashes, implementation snapshots and Core binary.
The command directories retain original stdout/stderr/status, and each
combination retains its existing-test and compatibility-case records.

The Core build record is **bb813b8b82e8b5636f82cde941354f9c8eea9948**, with
binary SHA-256
`cef1772b082521cda696905850d6edb69be82c0f45c9632bc268cd0a5628da96`.
Commit **3d887a460bb72064b29a6f9ea71293e2949a16d7** added the five
compatibility fixture/test/documentation files; its Core production sources
are byte-identical to that recorded build. The build identity remains bb813b8:
source equivalence does not change an executed command's `source_commit`.
The source-risk implementation and case hashes were independently checked
against the retained `tools.json` snapshots (164 file/hash checks).
The [certification identity index](https://github.com/musutrade/Harness-Gate/blob/d9a932b23dcfea709175285b594ca200450a9971/docs/quality/gh-288-certification-identity.json) records
the exact five committed blobs and hashes, per-combination implementation
hashes, Core subtree equivalence and retained evidence digests.

Projection requires and observes rejection for its missing independent callable
counter. Derive reports manual owners only. Both proc-attribute variants and
both feature-cfg variants reject at capture preflight. Exact cfg(test) disabled
is measurable; enabled retains the native mapping rejection. Supported capture,
plugin re-export and signed Core evaluation pass, including stale-context,
expiration, signature, replay and artifact-tamper negatives.

The two recorded measurement series differ; `baseline_adopted=false`.
The later process-cleanup changes in H
`1ea9a0a55fcd6690972f60a56e07e0bf9617b417` and merged main G
`e4a084be19de454cdaeebb44c41715fecf1f2b49` are outside this old Core build.
Their separate source/native regression evidence does not relabel this matrix
as a final-G Core certification. Rust 1.99 compiler-private native ABI remains
uncertified; the native choice below continues to require Rust 1.97.1.

## Fresh Core and the explicit runner

Build Core from the exact checkout being certified into an isolated target
directory. Preserve its real build command, exit, stdout/stderr and compiler
identity; do not use an installed or previously discovered `harness-gate`.
Provide a trusted build JSON with these required fields:

```json
{
  "binary": "/absolute/new-core-target/release/harness-gate",
  "sha256": "SHA256_OF_THAT_ACTUAL_BINARY",
  "source_commit": "EXACT_CHECKOUT_HEAD",
  "command": ["cargo", "build", "--locked", "--release", "--manifest-path", "/absolute/checkout/tools/harness-gate/Cargo.toml"],
  "exit": 0
}
```

The operator creates that record from the actual build, not from example values.
Additional compiler/build-source identity fields are retained verbatim. The
runner checks the supplied absolute binary path/hash and exact HEAD, requires a
successful locked build record, and retains it. The same current Core binary is
used for both source collector combinations; this does not change Core's own
compiler requirement or port the native collector.

```bash
python3 -B tools/quality/rust-source-risk/test_toolchain_compatibility.py \
  --evidence /absolute/new/gh288-evidence \
  --core /absolute/new-core-target/release/harness-gate \
  --core-build-record /absolute/retained/core-build.json
```

The evidence directory must be new and outside the checkout. The command runs
both required combinations; it does not silently omit an unavailable one.
It exits nonzero on any failed/blocked combination. `matrix.json` is updated as
results arrive; `success.json` is written only after every assertion passes and
the two real series IDs differ. No baseline is adopted. Unittest discovery does
not launch this expensive matrix automatically; its static contract test checks
the fixed pairs and diagnostic source boundaries. This opt-in arrangement does
not replace or skip an existing required CI test.

## Diagnostic cases and what they establish

| Case | Real diagnostic | Collector/Core boundary |
| --- | --- | --- |
| Projection closure | Exact original `closure_without_counter.rs` bytes compile, execute and export independently under both compilers | Requires the actual missing independent callable mapping rejection; never borrows the parent's counter |
| Derive, called and uncalled | Built-in `Clone` generated method is called or omitted while manual source functions execute | Manual owners alone may be measured; raw generated records are retained, with no generated coverage certification |
| Proc attribute, called and uncalled | A local std-only `proc_macro` crate emits a generated function; both execution variants retain exports | Real capture must reject the unsupported attribute before Cargo collection; no generic expansion or `expand_expr` |
| Feature cfg, off and on | Identical source compiles under explicit `feature="enabled"` cfg or its absence | Both capture preflights reject unsupported cfg; one configuration's AST cannot certify the other's ownership |
| Exact cfg(test), off and on | Identical source inventories exclude the test module; both native exports are retained | Actual native mapping boundary is recorded separately; AST exclusion is not proof that generated/test records map |
| Supported source/Core | Production `apps/server/src/lib.rs`, with an empty exact cfg(test) module, plus integration tests outside production roots | Real capture, independent plugin re-export, signed Core collection and evaluation with integrity negatives |

Unsupported compilation diagnostics are deliberately separate from collector
acceptance. A successful rustc invocation for a proc attribute or cfg case does
not mean capture succeeded. Conversely, source capture's preflight rejection
does not imply a compiler failure. Every case records the earliest measurement
boundary: LLVM export format, source inventory, source mapping or measured. If
LLVM 23 exports a format other than the currently required `3.1.0`, the runner
retains that format failure and cannot claim a downstream missing-counter check
passed. Unexpected supported-shape mapping failures are retained regressions,
not converted to unsupported success or filled with zero. Any necessary production
repair requires separate coordination and review beyond this five-file change.

The supported fixture follows the existing `acceptance.py` backend layout; its
integration test executes both manual branches. Capture is run in the selected
environment, and receipt rustc/LLVM hashes must match the prechecked combination.
The standalone plugin independently re-merges/re-exports the retained native
objects and profiles. Acceptance places a byte-identical copy of the supplied
fresh Core first in its test process PATH and checks the resolved path and hash.
Core's adapter intentionally clears its child environment; its plugin re-export
uses the receipt's exact absolute LLVM paths and this combination's pinned fresh
AST binary, without invoking cargo/rustc or resolving a default toolchain. The
test does not weaken that environment boundary to propagate ambient settings.

Core rehearsal preserves signed-binding collection, real supported-source
evaluation, stale-context, expiration, signature, replay and artifact-tamper
rejections. Existing synthetic CRAP limit comparisons are explicitly labeled
synthetic; they validate Core comparisons, not compiler counters. Unsupported
diagnostic cases do not claim to reach Core. The ephemeral signing key remains
outside the measured workspace and is deleted by the existing acceptance helper;
this local rehearsal is not a production signing authority.

## Retained evidence and identity

Every command has its exact arguments, relevant environment, cwd, status and
unmodified stdout/stderr. Failures are retained before raising. Per-case evidence
contains source/config/cfg identity, AST anchors/owners, original binary and raw
profiles, merged profiles, complete LLVM exports and native instance/count/region
records. Successful source metrics retain source SHA, owner spans, independent
instance counts, line/region denominators and exact rational CRAP. No regex,
macro string or generated branch is invented as a source decision. Rejected
metrics retain the precise exception or original CLI diagnostic. Existing test
mutations have distinct filenames and measurement records, preserving red bytes.

`tools.json` pins the actual rustc/cargo/LLVM paths and hashes, full `rustc -vV`,
fresh AST binary, copied implementation files and explicit Core build. Capture
receipts, independent exports, Core collection/evaluation, negative logs, series
and candidate success markers stay within the isolated evidence tree. Actual
results distinguish supported, unsupported, failed and blocked; missing data
does not become a favorable numeric value. A failed matrix has no success marker.

The unchanged plugin derives series from toolchain/pipeline identity and exact
implementation/analyzer hashes. Rust 1.97 and 1.99 reports are distinct series,
even when some metric values agree. Any base/head measurement must use the same
final plugin/analyzer/LLVM combination for both independent input snapshots.
Do not compare across compiler series, relabel old evidence, reset lineage or
automatically replace a baseline. Migration requires explicit review. Repository
required gates, 80/80/30 policy, source plugin's configured CRAP policy and all
debt/non-regression semantics remain unchanged.

## Selecting source or native

For manual business source decisions, select the source-risk collector explicitly
and use its supported AST/LLVM boundary with the exact reviewed tool combination.
For example, `RUSTUP_TOOLCHAIN=1.99.0 python3 .../capture.py ...` selects only that
child process; the retained matrix certifies only the supported shapes and exact
implementation identities above. Missing source counters and unknown owners still block measurement.

For generated functions/MIR ownership, explicitly select the existing
[native collector](native-external-toolchain.md). Its published runtime requires
the documented Rust **1.97.1** commit, compiler-private libraries and matching
LLVM **22.1.6**. Pass its explicit `--sysroot /absolute/rust-1.97.1` (or the
documented environment selector); installing 1.99 does not certify that driver.
This issue does not migrate it to 1.99, create a macro expander, revive #259,
switch engines automatically, or claim source and native CC/coverage equivalence.
