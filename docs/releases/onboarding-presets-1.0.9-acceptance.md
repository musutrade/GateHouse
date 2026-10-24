# onboarding-presets 1.0.9 compatibility receipt

Published 2026-10-24 by `musutrade/GateProbes`. Both execution-only products
retain every previous Core version and add exact Core 0.14.0 compatibility.
This is a preset release, not publication of Core 0.14.0 or expanded runtime
adapter/importer certification. Historical 1.0.8 remains immutable.

- [Release](https://github.com/musutrade/GateProbes/releases/tag/onboarding-presets-v1.0.9)
- [PR #84](https://github.com/musutrade/GateProbes/pull/84), head `3c12d205049d1fee8c0f17c2fad8227fc9086914`
- Tag/source merge: `07edfcd3c11245fe3e558f5fd44bdcfe2f6e39cf`
- Archive: `gateprobes-onboarding-presets-1.0.9.tar.gz`
- Archive SHA-256: `d472c8c09e68c0dedf1ab0a90fa67181c0d4d9d29be1292650babbe246ceed53`
- SBOM SHA-256: `5f3e20fdb5a8e38912e65e427430335e55cbb9bb407be65e545c7e50e9cf1ca6`
- SHA256SUMS SHA-256: `f3f80e497c4d464d092162bc0ece6384dcc7a5439cf2daa2483f51c263e16f62`
- Inventory SHA-256: `79b93c2611caa1fdcb3006374e3f42ae6cdfe929caeac97ef4cb8ff2518b000d`
- PR CI `40069275419`, main CI `40069590749`, release `40069937358`: attempt 1, success.

## Public artifact verification

All four integrity subjects (archive, SBOM, inventory and SHA256SUMS) passed
independent `cosign verify-blob` with exact certificate identity
`https://github.com/musutrade/GateProbes/.github/workflows/collector-release.yml@refs/tags/onboarding-presets-v1.0.9`
and issuer `https://token.actions.githubusercontent.com`. The publisher verified
all four provenance attestations; the receiving developer additionally verified
the archive attestation with signer workflow
`musutrade/GateProbes/.github/workflows/collector-release.yml` and checked its
source commit. Inventory hashes and source match the published tag. The archive
hash equals the locally accepted candidate archive.

## Downloaded onboarding acceptance

On Linux x86_64 the downloaded archive passed 29 real steps against each binary:

| Core | Origin | SHA-256 |
| --- | --- | --- |
| 0.13.0 | Downloaded published predecessor | `1800325ea2f541dd9b011c03dc93c3dac341e839ece6289ebe778275dcaacf75` |
| 0.14.0 | Source-built candidate, PR #184 `d89e529` | `0566ed841b7b4da978032c070c745d88786e4acc23721f45411ac5e26ee3b8de` |

The receiving developer repeated the actual Core CI `fetch_artifacts.py` path
with the updated pin and reran the extracted acceptance against the 0.14.0
binary above: all 29 steps passed. Checks include Rust/Python init/check/verify,
trust and tamper failures, upgrade and exact rollback. Downloaded Core 0.14.0
acceptance remains pending until that release exists.

The earlier [real Podman candidate receipt](0.14.0-candidate-acceptance.json)
binds another local build (`3bd78fc9…`). Its hash is not substituted for the
onboarding build above. The prerequisite handoff initially carried that earlier
hash; the committed onboarding evidence and actual rerun bind `0566ed84…`.

## Original failure retained

PR #184 at `d89e529` failed CI `40067868851` attempts 1 and 2 because pinned
1.0.8 did not declare Core 0.14.0 (`HGPLUGIN-CORE`). This repair pins the new
published artifact in `tools/compatibility/artifacts.json`; it does not relax
Core's exact-version check, rewrite 1.0.8, or change any quality threshold.
