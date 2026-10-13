# Native collector with external dependencies

The selected delivery route retains the native engine and uses Core's release
process. [Version `0.1.0-rc.5`](https://github.com/musutrade/Harness-Gate/releases/tag/rust-collector-v0.1.0-rc.5)
is published for Core's four-platform matrix. Source version `0.1.0-rc.6` wires
the project-owned CRAP policy into direct evaluation; every target must pass its
real packaged acceptance before publication.
Stable remains an archived candidate. This supersedes
stable rewriting as a delivery prerequisite, under the
[policy amendment](../engineering-policy.md#native-delivery-amendment-2026-09-14).

## Product and dependencies

The release contains the platform executables below, a separate CycloneDX SBOM
for each executable, Core-format `release-inventory.json`, `SHA256SUMS` and Sigstore signatures and
certificates. The binary embeds our driver, Python adapter sources, schemas and
license notices. It does not contain Rust, LLVM, Python or a system environment.
`--version` and `--licenses` need no external interpreter and write no cache.

| Platform | Target | Executable |
| --- | --- | --- |
| Linux x86_64 GNU | `x86_64-unknown-linux-gnu` | `harness-gate-rust-collector-linux-amd64` |
| macOS Intel | `x86_64-apple-darwin` | `harness-gate-rust-collector-macos-amd64` |
| macOS Apple Silicon | `aarch64-apple-darwin` | `harness-gate-rust-collector-macos-arm64` |
| Windows x86_64 MSVC | `x86_64-pc-windows-msvc` | `harness-gate-rust-collector-windows-amd64.exe` |

Each SBOM is named after its executable, removing `.exe`, then appending
`.sbom.cdx.json`. The runtime requires Python 3.12 or newer and an explicit
external Rust **1.97.1** for the same host target, commit
`8bab26f4f68e0e26f0bb7960be334d5b520ea452`, with matching `librustc_driver`,
LLVM **22.1.6**, sysroot and `llvm-tools-preview`. Target projects also need
their normal linker, native libraries and test services. `rustc-dev` is a
publisher build dependency; the consumer does not need its development crates.

Select the sysroot using `--sysroot` or `HARNESS_GATE_RUST_SYSROOT`.
`HARNESS_GATE_PYTHON` can select an interpreter for direct use; otherwise the
launcher calls `python3` from PATH (`python.exe` on Windows). Python runs isolated
with UTF-8 enabled. No command installs dependencies or changes
the default Rust toolchain. An incompatible/missing tool or a loading failure
blocks collection and produces `measurement_error`.

The release matrix uses Ubuntu 24.04, macOS 15 Intel, macOS 15 Apple Silicon and
Windows Server 2025, with Python 3.12 and matching native Rust hosts. The dependency
probe loads the actual driver and authenticates the matching compiler/LLVM files.
Only a successful target job establishes its hosted result; Linux acceptance
does not stand in for another platform. Platform and tool identities remain part
of the measurement evidence and cannot silently replace an existing baseline.

## Install and collect

After a tagged release exists, download the exact `rust-collector-v<VERSION>`
assets from the GateProbes releases. Use the explicit identity provisioning
below: verify Sigstore against the exact tag and the GateProbes workflow
identity, check `SHA256SUMS`, and verify the inventory/provenance before copying
the executable to a private installation directory:

```text
https://github.com/musutrade/GateProbes/.github/workflows/native-collector-release.yml@refs/tags/rust-collector-v<VERSION>
```

A different repository or workflow, a new download URL or a moved repository
must never silently replace this identity. `install.sh --rust-only` provisions
the same identity for its default version.

For example, the per-binary check uses the same Sigstore mechanism:

```sh
cosign verify-blob \
  --signature harness-gate-rust-collector-linux-amd64.sig \
  --certificate harness-gate-rust-collector-linux-amd64.crt \
  --certificate-oidc-issuer https://token.actions.githubusercontent.com \
  --certificate-identity "https://github.com/musutrade/GateProbes/.github/workflows/native-collector-release.yml@refs/tags/$TAG" \
  harness-gate-rust-collector-linux-amd64
sha256sum --check SHA256SUMS
```

`TAG` must be the explicitly selected release tag. Authenticate every inventory
subject using its `.sig`/`.crt`, as the workflow does. An unsigned local build is
for development acceptance, not a published release or trusted baseline.

Then run the installed binary:

```sh
harness-gate-rust-collector doctor --sysroot /absolute/external/rust-1.97.1
harness-gate-rust-collector capture \
  --sysroot /absolute/external/rust-1.97.1 \
  --source /absolute/project/Cargo.toml --sample contract \
  --output /absolute/evidence/new-head
harness-gate-rust-collector certify \
  --sysroot /absolute/external/rust-1.97.1 \
  --evidence /absolute/evidence/new-head/raw --anchor HOST_RETAINED_SHA \
  --output /absolute/evidence/head-report.json
```

Output directories must be fresh and outside the measured project. Repeat
`--sample` for the project's explicit integration-test targets; repeat `--feature`
for selected features. The capture prints its manifest SHA for separate retention
by the host. Do not reseal damaged evidence or adopt an anchor from untrusted
evidence. Certification re-merges raw profiles and independently re-exports LLVM
data from retained binaries. It does not rerun archived programs.

The cache defaults to `$XDG_CACHE_HOME/harness-gate/native` or
`$HOME/.cache/harness-gate/native`; `HARNESS_GATE_NATIVE_CACHE` can override it for
direct use. Windows defaults to `%LOCALAPPDATA%/harness-gate/native` when no explicit
cache is selected. Each payload has a separate content-addressed directory. Extra,
modified or symlinked payload files block execution rather than being repaired
silently. Preserve the original executable, cache and external dependencies for
historical re-export. Upgrades add another payload; rollback selects the previous
verified binary and its matching evidence. Neither operation rewrites a baseline.

## Measurement and Core contract

The native MIR instrumentation, counter aggregation, complexity, CRAP and Core
policy rules remain the existing algorithms. Host-specific tool discovery,
library loading, executable suffixes and path separators are handled by the
adapter; these changes do not introduce a new measurement formula.
Independent typed-MIR basic-block counters cover the old validated compiler-owner
scope, including derived methods, macro expansions, constructors, closures and
async bodies. The [native engine contract](https://github.com/musutrade/GateProbes/blob/155b233fb2f525f520b4fcc0ed73e017154345e0/plugins/rust-native-driver/README.md)
defines the precise limits. This delivery does not certify arbitrary macros or
projects beyond that contract.

Regions remain MIR blocks, complexity remains the typed normal CFG rule, and
CRAP uses the same exact rational formula. Core evaluates the project-configured
CRAP ceiling (the reference preset remains 30), required coverage,
changed-function rules and historical debt/non-regression decisions. No macro
exemption policy is enabled. Missing counters, uninstantiated generics, ambiguous
owners, incompatible inputs and evidence damage remain blocking errors. Reports
retain `mapping_complete_for_declared_scope`, `backend_complete`, source inventory
and exclusions; a fixture report does not claim complete Cargo/backend coverage.

Packaging paths, driver bytes and external tool identities remain real identities.
The old anchors and baselines stay intact; continuity across a different tool path,
build or engine needs explicit comparison/migration. There is no automatic history
reset or switch to the stable engine.

`collect` implements Core's existing protocol v2 and digest-pinned project capture
binding. Core authenticates the actual standalone executable, full arguments,
environment, context and nonce. Use explicit `--sysroot` in signed arguments and
signed PATH/XDG_CACHE_HOME values; Core reserves `HARNESS_GATE_*` environment keys.
Capture and Core collection must use the same cache and tool paths. The embedded
payload is covered by the executable digest, so no separate private-runtime receipt
or second delivery-signing packet is required. Old bundled delivery receipts are
rejected by this product. The host still owns trusted capture anchors and series
acceptance; an invocation signature does not independently attest the producer.

`certify` emits measurement facts and exits zero for valid evidence, even when the
numbers would fail policy. `evaluate` requires `--repository-root` and
`--policy-binding`, resolves the existing project policy file, and calls Core for
anchored base/head captures. See [the exact configuration fields and command](native-crap-policy.md).
Raw facts have no
legacy `passed` verdict fields. `collect` validates all selected owners before
emitting evidence and checks Core's invocation markers; errors exit nonzero.

`classify` retains the old per-file reasons and exits nonzero when classification
is incomplete. For a source excluded by selected features, provide an independently
anchored alternate-feature capture with `--witness` and `--witness-anchor`. The
existing classifier checks its compatibility before using that selection proof;
an unloaded source without such proof is never declared to have no runtime code.

## Release boundary

`.github/workflows/native-collector-release.yml` builds and tests the actual
standalone asset on pull requests and main. A `rust-collector-v*` tag additionally
uses Core's `release_policy.py`: exact manifest version/tag commit, protected-main
ancestry, and successful required CI for that exact commit. Publication requires
the native acceptance job and Core's protected `release` environment. It reuses
Core's inventory/checksum tooling, Sigstore signing, provenance verification and
immutable `gh release create`; existing releases cannot be overwritten.

There is no private candidate signing round, bundled-runtime installer or second
approval packet. The old bundled workflow stays suspended. See the
[build and acceptance commands](https://github.com/musutrade/GateProbes/blob/155b233fb2f525f520b4fcc0ed73e017154345e0/plugins/rust-native-plugin/README.md)
and the [local validation record](native-external-toolchain-validation.md).
