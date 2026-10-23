# onboarding-presets 1.0.8 compatibility receipt

Published 2026-10-23 by `musutrade/GateProbes` as an immutable GitHub release.
This receipt binds the Core 0.13 compatibility declaration; it is not a Core
release and does not claim official Core 0.13.0 download acceptance.

- Release: https://github.com/musutrade/GateProbes/releases/tag/onboarding-presets-v1.0.8
- Source merge: `54b98aa1745135231167e25f2b1b281cc7ac012f`
- Archive: `gateprobes-onboarding-presets-1.0.8.tar.gz`
- Archive SHA-256: `87cd7c8107a1082d3ff3113d14d4402abd241e651d168262038f569abfd89dc1`
- SBOM SHA-256: `fc22d85e7443600e279f2cbbc7e4d296e2f20d589f4ff42092b22395fa87696d`
- SHA256SUMS SHA-256: `40f6ffb8b3861b1bbc4bedea4da2a5f62cf22256307bfe8dc032a0e7469aac48`
- Inventory SHA-256: `94582ede9d4f909fad4524625e59107a6d42250ee2b7977a9d826abd0ed9f00a`
- PR: https://github.com/musutrade/GateProbes/pull/81
- PR CI: run `39981120637`, attempt 1, success
- Main CI: run `39982110007`, attempt 1, success
- Release workflow: run `39983651606`, attempt 1, success

## Independent verification

All four subjects (archive, SBOM, SHA256SUMS and release inventory) passed
`cosign verify-blob` with certificate identity
`https://github.com/musutrade/GateProbes/.github/workflows/collector-release.yml@refs/tags/onboarding-presets-v1.0.8`
and OIDC issuer `https://token.actions.githubusercontent.com`. All four passed
`gh attestation verify` with signer workflow
`musutrade/GateProbes/.github/workflows/collector-release.yml`. `sha256sum -c
SHA256SUMS` passed. The archive has ten regular files with mode 0644, uid/gid 0
and mtime 0; the SBOM file hashes match the extracted bytes.

The downloaded archive's 29-step Rust/Python onboarding acceptance passed against
the candidate Core 0.13.0 binary with SHA-256
`66288b97e76b0a516ca66ebdf58abebfa5b927e775eb9c5a0533bf48f085e1de`.
Evidence is retained at
`/mnt/dev-ssd/gatehouse-architecture-handoff/onboarding108-downloaded-acceptance.log`
and `/home/gem/gh152-onboarding-release-verify/acceptance-work/report.json`.

Core now fetches this exact archive for its real onboarding CI. The official
downloaded Core 0.13.0 acceptance remains pending the subsequent Core release.
