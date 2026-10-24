# onboarding-presets 1.0.10 compatibility receipt

Published 2026-10-24 as [onboarding-presets-v1.0.10](https://github.com/musutrade/GateProbes/releases/tag/onboarding-presets-v1.0.10). Both execution-only products retain all prior declarations and add exact Core 0.14.1 compatibility.

- [PR #85](https://github.com/musutrade/GateProbes/pull/85), head `b6b9ad15be1471c66cb3d86de51c8d4e252f3dfd`
- Immutable tag/source merge: `bbd7a3d5170b9c110ea362b1e1be212b327ae453`
- PR CI `40073620335`, main CI `40073921409`, release `40074225550`: attempt 1, success.
- `gateprobes-onboarding-presets-1.0.10.tar.gz` SHA-256: `0aca0d1fdaf3ac0c13775fdcd4b8076d942ff3e050a1fd8f017b880ccaed29c9`
- `gateprobes-onboarding-presets-1.0.10.tar.gz.sbom.cdx.json` SHA-256: `3f2a53893839b948160b7886d830caf198d11b42e195aeda1b678a43cdc0ac9d`
- `SHA256SUMS` SHA-256: `0b6872bdb8f070c3b632e2caa1d73b58f37d2936e2a516b87e2fb9ed298ff656`
- `release-inventory.json` SHA-256: `4cea46b6bb5d1498379162d2a1cd9993bb4249705da59fbc8d97a02dc7affa05`

## Verification

The archive matches the locally accepted candidate. All four integrity subjects passed publisher and independent receiving-developer signature verification with exact certificate identity
`https://github.com/musutrade/GateProbes/.github/workflows/collector-release.yml@refs/tags/onboarding-presets-v1.0.10`
and issuer `https://token.actions.githubusercontent.com`. The publisher verified all four provenance attestations; the receiving developer additionally verified the archive with signer workflow `musutrade/GateProbes/.github/workflows/collector-release.yml` and exact source commit. Inventory hashes and tag/source match.

The downloaded archive passed 29 real Rust/Python steps with each binary:

| Core | Origin | SHA-256 |
| --- | --- | --- |
| 0.13.0 | Published downloaded predecessor | `1800325ea2f541dd9b011c03dc93c3dac341e839ece6289ebe778275dcaacf75` |
| 0.14.1 | Isolated source-built candidate at `9324f9337b8c9c41728ca85967cd2242ff0589af` | `d049a0f593c05cc2f34cb090ab1dda7380f2eaa4d20491b2561c00d148b28056` |

Checks cover init/check/verify, trust/tamper negatives, upgrade and exact rollback. Core CI now fetches this exact archive and derives its execution prefix from the same pin. The receiving developer repeated that download and extracted acceptance path against the isolated candidate.

This does not certify a downloaded Core 0.14.1 artifact before publication or expand runtime/importer compatibility. Previous preset releases, failed Core v0.14.0 tag and original candidate receipts remain unchanged.
