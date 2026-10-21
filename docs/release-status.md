# Published releases and acceptance

## Core 0.8.0 installed-host candidate

The installed command is merged but not yet released. See [0.8.0 scope](releases/0.8.0.zh-CN.md).
Onboarding1.0.3 publication and pinning are complete (receipt below). Protocol0.1.1 and
Core0.8.0 must be verified from their final tag; no candidate build is release evidence.
The separate importer artifact and downloaded fixed-pair acceptance remain pending.

## Core 0.7.1 registry repair delivered

Core0.7.1 source `26734c7055870c1c9c4014bf75039d55b8c91ad8` completed release run
39487417615, including protocol0.1.0 and Core crate publication. Downloaded Linux
SHA256 is `63e7ebb3f57f0e864ce5c6f41bed00b6bbfbff64863a913df7b75652af1bf613`.
The prior candidate chronology follows; it does not describe current publication status.

Core 0.7.0 GitHub assets were published from `6242228a16f951bc30a881641c853afefae262a8`;
its crates.io job failed because the runtime protocol dependency was not publishable.
The tag and assets remain immutable. Core 0.7.1 repairs registry packaging and adds
protocol publication and Core package preflight before GitHub asset publication.
Onboarding presets 1.0.2 is published from `85b1856dd7061bfb19683df3921060b49490e5e2`
and pinned by Core CI. Its downloaded archive passed 29-step acceptance with
released 0.7.0 and source-built 0.7.1; the latter is candidate evidence only.
No real daemon acceptance, engine switch, lease migration or #62 completion is
claimed. See [the repair release plan](releases/0.7.1.zh-CN.md).

## Postgres externalization delivered, 2026-10-14

GateHouse #24 is accepted with published Core 0.5.0 and GateProbes Postgres 0.1.1. Independent public inventory/signature/provenance verification, 47 real service/doctor/verify contract records and all three installed presets pass. See the [final fixed-pair receipt](releases/postgres-service-0.1.1-acceptance.md). Historical tags, failed publication 0.1.0 and original consumer evidence remain unchanged.

## Core 0.5.0 public delivery, 2026-10-14

[Immutable v0.5.0](https://github.com/musutrade/GateHouse/releases/tag/v0.5.0), crates.io and fresh Linux binary/registry installation pass. All 21 assets, seven exact-tag signature/provenance subjects and source bindings were independently verified. Exact-main CI includes all three performance jobs; raw risk has zero failures and critical paths are 14/14. See [the public receipt](releases/0.5.0-public-verification.md) and [migration guide](releases/0.5.0.zh-CN.md). Postgres checks and legacy migration now belong to GateProbes; old configurations require explicit migration. Quality policy, accepted baselines and historical releases are unchanged.

## Core 0.4.9 public delivery, 2026-10-13

**GitHub release and public verification: pass.** The first split Core release
is published as [v0.4.9](https://github.com/musutrade/GateHouse/releases/tag/v0.4.9)
from `4d5b15a24c91a17b4823ec1626048cdfb20834da`, after exact protected-main
[CI 38385214782](https://github.com/musutrade/GateHouse/actions/runs/38385214782)
passed. [Release 38389039570](https://github.com/musutrade/GateHouse/actions/runs/38389039570)
passed policy, four platform builds, release quality and signed GitHub publication.

All 21 public files match the release inventory. Seven subjects passed exact-tag
Sigstore identity and exact-source/signer provenance verification. Fresh Linux
installation passed version/digest checks and generic verification with complete
evidence. All three real GateProbes ecosystem presets initialized and passed
configuration validation. This is not additional measurement certification.

**crates.io publication and fresh Linux installation: pass (2026-10-14).**
After the publishing credential was configured and the protected `release`
environment approved, only the failed crate job was retried in
[attempt 2](https://github.com/musutrade/GateHouse/actions/runs/38389039570/attempts/2).
The public `harness-gate 0.4.9` crate matches its registry checksum, original tag
commit and 134 production source files. A fresh locked registry installation,
version check, generic init/config and `verify --all` pass with complete evidence.
The [crate receipt](releases/0.4.9-crate-public-verification.md) records the
exact checksum, package normalization and Linux scope. The original missing-token
failure remains in attempt 1 and the dated original public receipt; no tag,
GitHub asset, signature, baseline or measurement identity was changed.

For future tags, the protected `release` job checks that `CARGO_REGISTRY_TOKEN`
is present and nonblank before staging or publishing GitHub assets. The crate
job repeats this check and passes the credential to Cargo through its environment,
without embedding it in shell commands. This is a presence check; Cargo still
enforces registry permissions. Release approvals and quality gates remain required.
The original `v0.4.9` workflow remains unchanged in its immutable tag.

The accepted performance baseline and quality/debt semantics are retained. See
[the public receipt](releases/0.4.9-public-verification.md) and
[installation guide](releases/0.4.9.zh-CN.md). Historical assets remain unchanged.

## GateHouse source migration, 2026-10-13

Active Core development now lives in [GateHouse](https://github.com/musutrade/GateHouse).
The CLI/crate remains `harness-gate` at version 0.4.8; the source migration is not
a new binary release. The development tree includes external preset loading,
while the published 0.4.8 package retains its embedded presets.

Core tags through v0.4.8 retain Harness-Gate downloads and signing identities;
installer routing for v0.4.9 onward targets GateHouse. This routing is not a claim
that v0.4.9 has been published. Current repository installers select GateProbes
native RC8; historical tagged installers retain their original pins. See
[migration boundaries](gatehouse-migration.md), [clone/build/quality acceptance](gatehouse-migration-acceptance.md)
and [installation paths](quality/standalone-installation.md).

## Four independent GateProbes packages, 2026-10-13

TypeScript risk RC4, HTTP/JSON contract RC5, Rust source-risk RC7 and Rust
measurer 0.3.1 are now published under their independent GateProbes tags.
All four immutable releases passed public download, exact-tag signature,
exact-source provenance and fresh installation acceptance. See the
[release handoff and evidence](quality/independent-plugin-release-acceptance.md).
Core's next release no longer packages these plugins; historical releases,
Core's current consumption pin and the native RC8 chain remain unchanged.

## GateProbes native collector rc.8, 2026-10-12

The native Rust/MIR collector's signed publication moved to GateProbes, which
published the prerelease
[rust-collector-v0.1.0-rc.8](https://github.com/musutrade/GateProbes/releases/tag/rust-collector-v0.1.0-rc.8)
from `155b233fb2f525f520b4fcc0ed73e017154345e0` ([run 38218575101](https://github.com/musutrade/GateProbes/actions/runs/38218575101))
after the required `ci.yml` run on GateProbes main succeeded.

- Four platform binaries (Linux x86-64, macOS Intel/Apple Silicon, Windows
  x86-64) were built in release mode from the migrated source and accepted per
  platform against the pinned Core 0.4.8 binary digest: 23-case `accept.py` and
  zero-skip helper tests passed on every platform.
- Assets: the four binaries and SBOMs, `SHA256SUMS`, `release-inventory.json`
  and keyless Sigstore signatures/certificates for all ten integrity subjects,
  plus GitHub build provenance attestations. The signing identity is
  `https://github.com/musutrade/GateProbes/.github/workflows/native-collector-release.yml@refs/tags/rust-collector-v0.1.0-rc.8`.
- Independent local re-verification passed 10/10 `cosign verify-blob` against
  that certificate identity, 10/10 `gh attestation verify` against the signer
  workflow, the certificate SAN, `SHA256SUMS` and the exact inventory.
- `install.sh --rust-only` (and `--with-rust`) now download and verify this
  release from the GateProbes identity above; the default collector version is
  `0.1.0-rc.8`. Core binary installation keeps its Harness-Gate identity. No
  Core gate, threshold, baseline or consumption pin changed in this step.
- Real Linux x86_64 installation through the updated installer passed with the
  pinned cosign 3.1.3 verifier (`Verified OK` for `SHA256SUMS` and the binary);
  the installed digest
  `da9b52b7097d5b824db44af80b82a7a9f03f0e9ed8c90801cf8ba2e65ab69744` matches the
  signed `SHA256SUMS`, and the installed binary reports
  `harness-gate-rust-collector 0.1.0-rc.8`. A `--rust-version 0.1.0-rc.7`
  request fails closed on the GateProbes boundary instead of silently falling
  back to the previous repository.

## Public Linux x86_64 acceptance, 2026-10-11

Core [0.4.8](https://github.com/musutrade/Harness-Gate/releases/tag/v0.4.8)
was published from `938a34b27cda8c13a706ca22adcfda567453f1ab` with the pinned
**Rust measurer 0.3.1** package. This record covers only the checks below; it
does not claim installation acceptance for other Core platforms, the other
candidate packages, the native plugin or crates.io.

- **Platform:** Linux x86_64 (`Linux 7.0.0-38-generic x86_64`).
- **Core:** the tagged
  [install.sh](https://raw.githubusercontent.com/musutrade/Harness-Gate/v0.4.8/install.sh)
  (SHA-256 `a19180fadd6cfa9b86b29fdc70ad301484d30136bb57c22dcb2d40534dcc86eb`)
  was run in an isolated temporary directory as
  `bash install.sh --version v0.4.8 --install-dir <temp>` without
  `--with-rust`. The installer verified `SHA256SUMS` and the Sigstore keyless
  certificate for identity
  `https://github.com/musutrade/Harness-Gate/.github/workflows/release.yml@refs/tags/v0.4.8`
  using its checksum-pinned cosign 3.1.3 verifier. The installed binary reports
  `harness-gate 0.4.8` (SHA-256
  `888fa1527eae37e3163aac2e1479e91ec03a0c8380b3c49da6e9266575597418`).
- **Rust measurer:** downloaded from
  `https://github.com/musutrade/Harness-Gate/releases/download/v0.4.8/harness-gate-rust-measure-0.3.1-linux-amd64.tar.gz`.
  Its SHA-256 `09645a8b53075d89172efde67250e4bea2967f02184d4f8a5bfdfb389da0eac1`
  matches `tools/quality/rust-measure-release.json`, `SHA256SUMS` and the
  signed release inventory. `cosign verify-blob` with the same tag identity
  returned `Verified OK`. The extracted analyzer reports
  `harness-gate-rust-measure / 0.3.1 / mccabe-rust-3` through the packaging
  smoke check; the binary has no `--version` flag and acceptance does not use
  one.

## Verified corrected release, 2026-09-22

Core [0.4.7](https://github.com/musutrade/Harness-Gate/releases/tag/v0.4.7)
and [crates.io 0.4.7](https://crates.io/crates/harness-gate/0.4.7) are published
from `3e7b49f32b2b4afa79c8bf1edbb55ed5e2e96ee2`. The release includes the corrected
**Rust source-risk RC6** Linux archive and the byte-identical **TypeScript RC4**
and **HTTP JSON contract RC5** Node packages. The independently published
[native Rust/MIR RC7](https://github.com/musutrade/Harness-Gate/releases/tag/rust-collector-v0.1.0-rc.7)
remains the installer default and retains its original source identity.

All 69 current release assets (39 Core/package assets and 30 native assets),
checksums, exact-tag signatures and exact-source attestations were verified:
[Core 0.4.7 and packages](quality/release-0.4.7/published-verification.json),
[native RC7](quality/release-native-rc7/published-verification.json).
The [registry download](quality/release-0.4.7/crate-public-verification.json)
matches its registry checksum, version and tagged clean source.

The actual public Linux installation passed **82 runtime tests without skips**:
23 Core/native, 20 TypeScript, 19 HTTP contract and 20 Rust source-risk tests.
[Clean-install receipt](quality/release-0.4.7/public-install-verification.json).
The test harness uses the tagged repository's Python reference transport only
for the Node transport test; installed runtime modules come from the signed
public packages. Core evaluator code and metric formulas are unchanged.

Follow the [0.4.7 release and installation guide](releases/0.4.7.zh-CN.md).
The incomplete RC5 archive and its failed acceptance remain recorded below;
no published tag or asset was moved or overwritten.

## Packaging repair and published delivery, 2026-09-22

Core [0.4.6](https://github.com/musutrade/Harness-Gate/releases/tag/v0.4.6),
[crates.io 0.4.6](https://crates.io/crates/harness-gate/0.4.6), and native
[RC7](https://github.com/musutrade/Harness-Gate/releases/tag/rust-collector-v0.1.0-rc.7)
are published from `4ecde0305572e5439dc9e6513cd0fe2de2d73224`.
All 69 uploaded assets passed public inventory, checksum, exact-tag signature and
source-provenance verification: [Core and packages](quality/release-0.4.6/published-verification.json),
[native RC7](quality/release-native-rc7/published-verification.json),
[registry source](quality/release-0.4.6/crate-public-verification.json).

Public clean installation passed 23 Core/native tests, 20 TypeScript tests and
19 HTTP contract tests. The Rust source-risk RC5 archive failed two of its 20
runtime tests because `ast/Cargo.lock` was omitted. This is a package defect,
not a signature failure or a quality waiver. The
[partial acceptance record](quality/release-0.4.6/public-install-verification.json)
and [failed runtime output](https://github.com/musutrade/Harness-Gate/blob/d9a932b23dcfea709175285b594ca200450a9971/docs/quality/release-0.4.6/rust-source-public-tests.log)
retain that result. The existing release notes identify the defect; no asset or
tag was overwritten.

Core **0.4.7** / Rust source-risk **RC6** provide the corrected delivery. Its
[release guide](releases/0.4.7.zh-CN.md) documents the repair and exact installation.
Publication and public acceptance are complete, as recorded above. Native RC7, TypeScript
RC4 and HTTP JSON contract RC5 retain their identities. All collectors remain
independent RC candidates; no metric threshold or baseline is changed.

## Standalone delivery update, 2026-09-14

The native old-engine standalone [0.1.0-rc.4 release](https://github.com/musutrade/Harness-Gate/releases/tag/rust-collector-v0.1.0-rc.4)
is published for Linux x86_64 from `52532a7ed133dacf1f665e48e8580c37cdaa7cee`;
[release workflow 34823683270](https://github.com/musutrade/Harness-Gate/actions/runs/34823683270)
completed its signed inventory and provenance checks. It contains our compiled
plugin and metadata, with Rust/LLVM and Python supplied externally. This is distinct
from installer RC4, which delivered the historical bundled RC3 product below.

Core [0.4.3](https://github.com/musutrade/Harness-Gate/releases/tag/v0.4.3)
and native [0.1.0-rc.5](https://github.com/musutrade/Harness-Gate/releases/tag/rust-collector-v0.1.0-rc.5)
are published from `e5b329250cf35b57769bd6be78fa3da8159bd2a5` for Linux x86_64,
macOS Intel, macOS Apple Silicon and Windows x86_64. The native
[release run](https://github.com/musutrade/Harness-Gate/actions/runs/34842418510)
and Core [release run](https://github.com/musutrade/Harness-Gate/actions/runs/34844016671)
completed; Core 0.4.3 is also on crates.io. The public four-host inventories,
checksums, exact-tag signatures and source provenance were verified. The actual
installed Linux pair passed all 17 native/Core acceptance tests without skips.

Native [0.1.0-rc.6](https://github.com/musutrade/Harness-Gate/releases/tag/rust-collector-v0.1.0-rc.6)
is published from `caadbeb2a37fb0dc24a84b0d80ceeaa40f185aa2`. Its
[release run](https://github.com/musutrade/Harness-Gate/actions/runs/34865689734)
passed all 22 native/Core acceptance tests on each of the four platforms without
skips. All 30 public assets, checksums, signatures and exact-source provenance
were independently verified. It fixes
[project CRAP policy configuration](quality/native-crap-policy.md).

Core `v0.4.4` was tagged at the same commit, but its
[release audit](https://github.com/musutrade/Harness-Gate/actions/runs/34868049009)
blocked publication on newly published
[RUSTSEC-2026-0285](https://rustsec.org/advisories/RUSTSEC-2026-0285): the locked
`rustls 0.23.43` requires `>=0.23.45`. No Core 0.4.4 release or crate was published.
The tag and failed evidence remain immutable; no audit exception is added.

Core [0.4.5](https://github.com/musutrade/Harness-Gate/releases/tag/v0.4.5) was
subsequently published with `rustls 0.23.45` and native RC6 as its installer
default. Existing release records below retain their original identities.

## Bundled delivery acceptance, 2026-09-13

Verified on **2026-09-13**. Core 0.4.2 and its compatible optional Rust delivery are published and installed on the acceptance host. Engineering Policy semantics are unchanged.

| Component | Published version | Source / verification |
| --- | --- | --- |
| Core | [0.4.2](https://github.com/musutrade/Harness-Gate/releases/tag/v0.4.2) and [crates.io 0.4.2](https://crates.io/crates/harness-gate/0.4.2) | `718bc5a1ea82f8364f91fc72f179ea0db689f6b5`; [release workflow 34738554209](https://github.com/musutrade/Harness-Gate/actions/runs/34738554209) |
| Rust collector | [0.1.0-rc.3](https://github.com/musutrade/Harness-Gate/releases/tag/rust-collector-v0.1.0-rc.3) | Same source; [publication 34739430177](https://github.com/musutrade/Harness-Gate/actions/runs/34739430177); six signed assets, independent draft and public readbacks |
| Rust installer | [0.1.0-rc.4](https://github.com/musutrade/Harness-Gate/releases/tag/rust-collector-installer-v0.1.0-rc.4) | Same source; [publication 34739962352](https://github.com/musutrade/Harness-Gate/actions/runs/34739962352); compressed, cached delivery of RC3 |

[The downloaded crate](quality/release-0.4.2/crate-public-verification.json) also matches the registry checksum, release source commit and report implementation.

Core Linux SHA-256: `eda5179d96b32269124150980ffcaa7e57878605133bc6ead51e5aeb8a6ad0a0`. Crate checksum: `0cf22fddc34242c4e6eb59369573e0ba7fe994cc13ca70f9a19ffdabddf7784e`. Installer SHA-256: `fdd277b6ec48ec89b4155d0df5d2bf6f8e6c2b15c41f0ea711cdb7eee479298a`.

The root installer at immutable source `9ffa2b829ec25ca54fcb00a9bc720284c3913924` selects installer RC4. Historical Core tags retain their original installer pins and are never rewritten. Follow the [installation and offline guide](quality/rust-collector-installation.md).

## What was accepted

The first local attempt correctly failed execution because the operator service PATH omitted Cargo; its [failed acceptance receipt](quality/release-0.4.2/missing-cargo-service-environment.json) is retained. Only the launch environment was corrected. The unchanged exact release-build Linux artifact then completed Arc Admin full and actual staged hook acceptance before delegated publication approval. After publication, the public installer verified Core SHA-256 and Sigstore and installed the identical binary. The plugin's RSA/Sigstore, exact inventory, clean offline installation, selection/rollback and authenticated configured Core collection were verified. [Public install](quality/release-0.4.2/public-user-install.json), [artifact acceptance](quality/release-0.4.2/core-artifact-acceptance.json), [draft readback](quality/release-0.4.2/draft-verification.json), [public readback](quality/release-0.4.2/published-verification.json).

## Compatibility

The [RC3 reviewed matrix and incremental materials](https://github.com/musutrade/Harness-Gate/releases/tag/rust-collector-materials-718bc5a-v1) bind this exact Core 0.4.2 binary and Linux x86_64 host: glibc 2.43, kernel `7.0.0-31-generic`, library fingerprint `f9b213877a2ea9aa44e6119e6fe2e1361f84f9a8084abd410ab8ef34b74e2872`. This is exact tested ABI support, not general Linux certification.

The 1,061,498,880-byte runtime archive remains byte-identical. Compressed tools and plugin layers total about 282 MB; first use with bootstrap, verifier and metadata is about 444 MB. Verified unchanged layers are cached. New-version installation and Core binding are fresh checks; unchanged native capture and adversarial lifecycle/cryptographic receipts are explicitly reused, not relabeled as rerun. Installed disk usage remains larger than transfer size.

## Arc Admin and issues

[Full acceptance and scoped cost evidence](quality/arc-native-20260913/README.md) replace the old missing-input blocker: 27 execution checks pass, three producers supply 1,921 records, the real Git baseline resolves, all 5,760 decisions are retained and 33 report artifacts verify. Hook correctly reports full quality not_collected. Coverage/CRAP still cause actual **quality FAIL**; execution and delivery acceptance are not a quality waiver.

[#249](https://github.com/musutrade/Harness-Gate/issues/249) was completed by the earlier RC2/Core 0.4.1 delivery. The integration and documentation scope of [#215](https://github.com/musutrade/Harness-Gate/issues/215) is complete: receipts cover baseline, real full/hook, routing, environment/service and bounded original-topology cost. Arc Admin #40/#41 and [acceptance documentation PR #42](https://github.com/musutrade/arc-admin/pull/42) are merged and preserve all original cargo-flow gates. No stable collector promotion, policy weakening, automatic series migration or gate authority transfer is claimed.

## Retained history

Core 0.4.1 could not finalize the large native report and could not load staged partial host inputs. [PR #251](https://github.com/musutrade/Harness-Gate/pull/251) and [PR #253](https://github.com/musutrade/Harness-Gate/pull/253) ship both fixes in 0.4.2. The [old binary failure](quality/arc-native-20260913/released-core-large-report-failure.json) and candidate results retain their original identities.

Earlier [RC2 delivery record](https://github.com/musutrade/Harness-Gate/blob/34724396c9cec4ac14899d97a5145c8197bbfe5b/docs/release-status.md), [RC2 operator continuation](quality/release-rc2/operator-continuation.json), and [RC1 HTTP/2 recovery](quality/release-0.4.1/recovery-result.json) preserve failed runs and their independently verified continuations. Current collector uploads use HTTP/1.1 through the authorized local proxy.

[Original corresponding sources, notices and relink materials](https://github.com/musutrade/Harness-Gate/releases/tag/rust-collector-materials-7165558-v1) and [per-file engineering license review](https://github.com/musutrade/Harness-Gate/releases/tag/rust-collector-review-7165558-v1) remain applicable to identical payloads. The actual incremental reviewer is Codex. Private keys and Arc Admin business source are excluded from public handoff.

Onboarding1.0.3 prerequisite is published from `727dd742510afd01c22d91dc90d5ffe474300531`
(release run39554478429), archive SHA256
`d4d3889c199137b067845eac7869a372134e4e4b899895235d9cc2f0362b7337`.
Core CI now pins this immutable archive. Downloaded presets passed 29 steps with
released Core0.7.1 and source-built0.8.0; downloaded Core0.8.0 reacceptance remains pending.
