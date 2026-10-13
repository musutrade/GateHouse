# GH-228 private Linux runtime candidate

> Storage update (2026-10-13): historical raw logs/bundles are available through
> the [verified archive guide](archive.md). Dates, digests and outcomes below are unchanged.

GH-227 was accepted and merged through PR #233 at
`b627bbc0e202b6e8a40354b731bd1a4291f31d7a`, the preserved baseline of this branch.
The operator supplied the pinned compiler-private archive after the
[original dependency failure](blocker.md). Its verified SHA-256 is
`0109304e1995cce9e3362208f5d4ec0944e52a2ddfbc0a85d4ce5bea5d3081ab`.
[operator-bootstrap.json](https://github.com/musutrade/Harness-Gate/blob/d9a932b23dcfea709175285b594ca200450a9971/docs/quality/gh-228/operator-bootstrap.json) retains the successful build
command, compiler identity, workspace-local overlay and exit code. The original
download/build failures and operator tests remain retained; recovery did not
change the global Rust toolchain.

## Scope and commands

This implements the P2 runtime and P3 local bundle assembly. Each original item
was estimated at most three focused hours. Investigation/build waits are recorded
separately from implementation. No P4–P8 acceptance, compatibility receipt,
installation, signing, publication or GH-215 activation is claimed.

The bundle contains `bin/harness-gate-rust-collector` with these commands:

| Command | Contract demonstrated here |
| --- | --- |
| `doctor --json` | Checks the exact observed host and every private payload hash; reports runtime completeness separately from unconfigured delivery compatibility. |
| `certify --evidence RAW --anchor SHA` | Authenticates the original capture against the supplied anchor, re-exports with the original private LLVM tools, and returns measurement-only JSON with exit 0. Integrity errors return no measurement and exit 1. |
| `classify --evidence RAW --anchor SHA` | Performs native re-export and scoped file classification. Incomplete classification returns no usable measurement and exit 1. |
| `collect` | Strict development v1 JSON parsing, exclusive error response, diagnostics on stderr, exit 1 and no sampling for every currently unknown Core/protocol/ABI combination. |

The reviewed delivery matrix remains empty, as specified by GH-227. In particular,
`collect` is a rejecting command boundary, not working installed-host integration.
P6 must bind trusted host requests, exact context/artifact checks and the released
Core envelope before enabling a positive request. This candidate does not translate
the development envelope into a fabricated Core receipt. P2 native sampling is
exercised directly through the private measurement API, with commands retained in
the evidence archive. No release or policy fallback exists.

The launcher computes its root without changing the caller's working directory,
uses private Python with `-I -S -B`, removes Python overrides and `LD_PRELOAD`,
and restricts PATH to the bundle. Cargo, its workspace wrapper, rustc, LLVM,
the GCC linker driver, collect2, LTO helpers, linker, startup objects and link
libraries have private paths. Cargo metadata also receives private `RUSTC`.
Existing developer call defaults and legacy exit behavior are preserved.

`MeasurementComplete` and `MeasurementFailure` separate authenticated facts from
errors. The standalone response removes report/function `passed` and classification
`measurement_passed`/`baseline_accepted`; original retained reports are unchanged.
Core independently evaluates the same low-coverage measurements. The development
policy/lineage fixture builder is test input preparation only and is not shipped.
No accepted CRAP, coverage, debt, lineage, requiredness or capability rule changes.

## Inputs, imports and license inventory

[runtime-inputs.json](https://github.com/musutrade/Harness-Gate/blob/d9a932b23dcfea709175285b594ca200450a9971/docs/quality/gh-228/runtime-inputs.json) records 842 payload files, original
input paths and SHA-256 values, payload modes, 21 checksum-verified original
Cargo crate archives and their declared licenses/notice members, compiler commit,
Python version, operating-system package versions, extension imports and recursive
ELF dependency output. The builder itself, native-driver sources/Cargo.lock,
operator build record and rustc-dev archive are pinned as build inputs.
Assembly validates every source byte before creating the destination and refuses
to overwrite an existing destination or input lock. The runtime inventory excludes
source paths; it is not the future signed delivery manifest.

[import-inventory.json](https://github.com/musutrade/Harness-Gate/blob/d9a932b23dcfea709175285b594ca200450a9971/docs/quality/gh-228/import-inventory.json) covers all selected local modules
and conditional imports. Only observed Python extension modules are included;
interactive, site/user, database and optional network Python facilities are outside
this runtime's command scope. All selected imports and native paths run in tests.
[frozen-helpers.json](https://github.com/musutrade/Harness-Gate/blob/d9a932b23dcfea709175285b594ca200450a9971/docs/quality/gh-228/frozen-helpers.json) verifies the four frozen C-module hashes.
Their mixed-module validation helpers keep the roles in
[python-retention.md](../python-retention.md); their generic policy functions are
never an authoritative product execution path. `rust_native_policy` is inventoried
as development-only and absent from the bundle. The prior import inventory is
preserved in `import-inventory-before-runtime.json`.

| Shipped component | License material retained in the bundle |
| --- | --- |
| Harness-Gate Python and native driver | `licenses/harness-gate/LICENSE` (MIT); native source and build-input hashes in the lock. |
| Rust compiler, standard library, LLVM and Rust linker | `licenses/rust/`, including Rust license files and `COPYRIGHT.html` with upstream third-party notices. |
| Cargo | `licenses/cargo/`; Rust distribution notices also retained. |
| Driver's 21 registry dependencies | Original `.crate` bytes under `licenses/crates/`; Cargo.lock checksum, license expression and embedded notice paths recorded for each. |
| Python 3.14.4 and native extension dependencies | Versioned package copyright files under `licenses/<package>/copyright`. |
| GCC 15, binutils, glibc link inputs, libgcc/libstdc++ and recursively resolved shared libraries | Versioned package copyright files, including GPL/LGPL and runtime-exception notices; complete common license texts under `licenses/common/`. |

The package list includes every selected distro runtime dependency. Original
upstream notice files are copied without rewriting their terms. This is an
inventory and retained-notice closure for this local candidate, not authorization
to distribute it. Before publication, supply-chain work must provide the applicable
corresponding-source distribution/offer and other obligations for the selected
GPL/LGPL binaries; notices and hashes alone do not fulfill that work. No signed
SBOM, source offer or legal clearance is asserted by this PR.

## Verified host boundary

Only Linux x86_64, kernel `7.0.0-31-generic`, glibc `2.43` and the exact host
library and `/bin/sh` hashes recorded in the lock were exercised. The dynamic
loader and glibc family plus the launcher shell are explicit host dependencies;
Python, Rust, Cargo, LLVM and the linker tools are private. This is intentionally
an exact observed host constraint, not a minimum-glibc or other-platform claim.
Compiler identity is rustc 1.97.1 commit
`8bab26f4f68e0e26f0bb7960be334d5b520ea452`, LLVM 22.1.6.

Native tests run from retained fixture directories with only bundle `bin` on PATH,
an empty workspace-local Cargo home, offline dependency resolution and no ambient
Python module path. Project dependencies are separate caller inputs: the Cargo
fixture's original locked Tokio and pin-project-lite archives were staged and
vendored locally; [project-inputs.json](https://github.com/musutrade/Harness-Gate/blob/d9a932b23dcfea709175285b594ca200450a9971/docs/quality/gh-228/project-inputs.json) records them. No
general C/C++ build-script toolchain or arbitrary project dependency cache is
promised. Unsupported external commands must fail rather than discover host tools.

This checkout still exists on the host. These tests demonstrate private executable
selection and private imports; they are not P7 clean-host/OS isolation acceptance.
The attempted syscall trace was denied by the sandbox and remains a failure record.
Retained debug binaries may contain original build paths as metadata; execution
does not use those paths to load modules or tools. Measurement identities still
include tool paths. Moving a bundle cannot establish series equivalence or rewrite
old capture paths, anchors or baselines.

## Build and comparison

Executed inventory command from the assigned checkout:

```sh
python3 tools/quality/build_rust_collector.py --inventory --driver target/gh-228/operator-native-runtime/build/debug/harness-gate-rust-native-driver --sysroot "$(rustc --print sysroot)" --crate-cache target/gh-228/crate-inputs --rustc-dev target/gh-228/operator-native-runtime/rustc-dev-1.97.1-x86_64-unknown-linux-gnu.tar.xz --build-record target/gh-228/operator-native-runtime/bootstrap.json --lock target/gh-228/runtime-inputs-v6.json
python3 tools/quality/build_rust_collector.py --lock target/gh-228/runtime-inputs-v6.json --output target/gh-228/bundle-v6-one
python3 tools/quality/build_rust_collector.py --lock target/gh-228/runtime-inputs-v6.json --output target/gh-228/bundle-v6-two
```

All three exited 0. [build-comparison.json](https://github.com/musutrade/Harness-Gate/blob/d9a932b23dcfea709175285b594ca200450a9971/docs/quality/gh-228/build-comparison.json) records two
996,761,600-byte tar files with identical SHA-256:
`357ad48de6f2387363a00bfcf1d1bed81518cc84ed1a763bdbe146d828ff4548`.
Assembly normalizes ordering, timestamps, owner/group and directory/file modes.
Earlier v5 assemblies also matched across umasks 0077 and 0022, but are superseded
by v6's private Cargo metadata fix. Original candidates remain under target.

This compares two assemblies of the same pinned prebuilt inputs, not two Rust,
Python or native-driver compiler builds. Compiler rebuild reproducibility is
unproven. Fresh captures differ in paths, profiles, anchors, commands and timing;
these differences are retained, never normalized into measurement equivalence.

## Retention and validation

See [validation.md](validation.md) for actual checks and genuine failures.
The committed native evidence archive (nineteen parts listed in
[retained-evidence.json](https://github.com/musutrade/Harness-Gate/blob/d9a932b23dcfea709175285b594ca200450a9971/docs/quality/gh-228/retained-evidence.json)) retains 14
verified native capture manifests and complete original fixture/cargo captures: binaries, profiles, LLVM exports, native inventories, source bytes,
tool identities, manifests, anchors and Core evaluation inputs/outputs. No
hash-only or reconstructed native positive substitutes for those bytes.
[retained-evidence.json](https://github.com/musutrade/Harness-Gate/blob/d9a932b23dcfea709175285b594ca200450a9971/docs/quality/gh-228/retained-evidence.json) inventories every archived file
and records archive hashes; retention verified the archived bytes against their
originals. See [transport.md](transport.md) for byte-preserving restoration and
the submission transport limitation. [resumed-validation-logs.tar.gz](archive.md) retains
complete resumed logs, including the failed intermediate runs.

The roughly 1 GB runtime tar files, compiler-private archive and intermediate
build trees remain at the exact workspace paths in the lock/comparison. They are
not committed or published. The controller must retain this assigned workspace
until copying those artifacts to its durable issue-artifact store with the recorded
hashes; no remote artifact upload is claimed. The committed capture/log archives
are the durable review record for this PR. Later clean-host acceptance must create
fresh captures in its own assigned checkout and retain its own full runtime;
it must not rely on historical external workspaces. Git history retains this PR's
committed evidence through the delivery chain; do not replace it with digest lists.
