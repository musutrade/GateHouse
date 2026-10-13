# Independent plugin publication and Core release handoff

Status: **pass**, 2026-10-13. All four remaining collectors published from
GateProbes commit `f9834a988e9e55ec99b7ebe81fb67a9ca16b2928`, after its
[exact main CI](https://github.com/musutrade/GateProbes/actions/runs/38337521708)
passed. The [machine receipt](independent-plugin-release-acceptance.json)
records the source, tags, public assets, digests, signing identities, verification
commands and installation results.

| Plugin | Independent release | Successful publication run | Public-package acceptance |
| --- | --- | --- | --- |
| TypeScript risk | [0.1.0-rc.4](https://github.com/musutrade/GateProbes/releases/tag/typescript-risk-v0.1.0-rc.4) | [38339077287](https://github.com/musutrade/GateProbes/actions/runs/38339077287) | Fresh production install, 21 tests, pinned Core decision contract |
| HTTP/JSON contract | [0.1.0-rc.5](https://github.com/musutrade/GateProbes/releases/tag/http-json-contract-v0.1.0-rc.5) | [38338985149](https://github.com/musutrade/GateProbes/actions/runs/38338985149) | Fresh production install, 21 tests, pinned Core collector contract |
| Rust source risk | [0.1.0-rc.7](https://github.com/musutrade/GateProbes/releases/tag/rust-source-risk-v0.1.0-rc.7) | [38338987066](https://github.com/musutrade/GateProbes/actions/runs/38338987066) | Extracted archive, 26 tests without skips, pinned Core metric contract |
| Rust measurer | [0.3.1](https://github.com/musutrade/GateProbes/releases/tag/rust-measure-v0.3.1) | [38338989282](https://github.com/musutrade/GateProbes/actions/runs/38338989282) | Extracted executable identity and pinned Core AST consumer contract |

## Trust and installation evidence

All four releases are immutable. Their 48 public files match the four exact
upload inventories. Independently downloaded bytes passed 16/16 Sigstore
signature checks and 16/16 GitHub provenance checks. Each check binds the exact
plugin tag, source commit and GateProbes `collector-release.yml` identity, with
GitHub's OIDC issuer. Checksums, sizes and the package/SBOM set match the signed
inventories. Deliberately using the old Core repository or a wrong plugin tag
fails signature verification.

The public packages were accepted again after download and signature verification,
using the packaging acceptance helpers at the source commit above. Node runtime
dependencies must resolve inside the fresh production install; withholding the
installed TypeScript dependency correctly fails both Node CLIs. Only the test
harness imports the separate Istanbul development tool. Rust source-risk runs
with Rust 1.97.1; the installed inventory executable is taken from the verified
archive. No Core or plugin binary is rebuilt for this public installation check.
The measurer has no `--version` flag; its actual analyzer/version/rule response
and AST consumer contract are checked instead.

The receipt preserves the fixed Core 0.4.8 binary digest and its separate Python
consumer source pin. Acceptance is Linux x86_64 with Node 24.18.0 and the stated
Rust toolchain, not new platform or measurement certification. Hosted packaging
also passed the existing source tests (including 12 source-risk and 7 measurer
Cargo tests) before signing. The initial local provenance command used mutually
exclusive CLI flags and failed before verification; the corrected exact
certificate-identity command in the receipt passed every subject.

## Core release change and rollback

Core `release.yml` now builds and publishes its four platform binaries, Core
SBOM and signed metadata. It no longer builds, packages or publishes these four
plugins, and publication depends on the unchanged policy, build and quality jobs.
Inventory, signing, provenance and crates.io behavior retain their existing
Core authority. The workflow regression test requires exactly those four binaries
and one Core SBOM and rejects a plugin packaging dependency.

Historical Core releases and their plugin assets remain available. Core CI keeps
its existing verified rust-measure 0.3.1 pin in Core v0.4.8; consuming the newly
published bytes requires a separately reviewed pin change. This handoff does not
silently replace that digest. Existing users can continue selecting the historical
artifacts for rollback, or select the independent GateProbes tags explicitly.
Reverting the Core workflow change restores its previous packaging job without
rewriting either repository's tags, releases or signatures.

The legacy packaging helper and release/compatibility tests remain for their
current consumers and rollback. Ordinary CI independence, ecosystem preset
externalization, broader source/fixture cleanup and repository naming remain
OpenSpec 6.1/6.3/6.4. The paused bundled-runtime RSA/Sigstore product retains its
4.3 disposition and is not enabled by these publications.
