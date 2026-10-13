# Pending execution and retention handoff

Prepared against main `5d9bec3dc3f0cd587b943316f7af8811c9f580bf`.
GH-239 remains open. Engineering Policy semantics are unchanged. This document
requests no immediate production execution and grants no publication authority.

## Observed scheduling boundary

On 2026-09-11, organization runner group 4 was read back as
`harness-gate-rust-collector-release`, restricted to exactly
`musutrade/Harness-Gate/.github/workflows/rust-collector-release.yml@refs/heads/main`.
Its runner count was zero. The approved group needs no recreation. The workflow
remains a review template at `tools/release/rust-collector-release.production.yml`.

A real job cannot be certified by the existing local startup probe. Before the
GH-240 execution, retain the reviewed workflow SHA, runner image/archive digest,
read-only input inventory, packet path/digest, resource/network policy and
external diagnostic destination. Register a fresh ephemeral organization runner
in group 4 with only the `rust-collector-release` label after verifying the group
restriction and queued jobs again. Do not register a general developer host.

GH-240 must retain actual run/job/runner IDs, exact source, protected approval,
ABI/input verification, real signing receipts and negative verification results.
After execution, retain runner diagnostics outside the disposable host, verify
runner removal through the organization API and record host teardown. A retry
uses a fresh host. Registration and real OIDC signing remain pending approved
production execution, as required by GH-239's existing scope.

## Concrete proposed GitHub ordering

The current requirement for published immutable evidence before the completed
GH-239 approval packet conflicts with GH-239's prohibition on tags/releases.
The proposed resolution is an explicit GH-240 preflight evidence publication
phase, followed by final packet verification and a separate collector publication
approval. This proposal is not an accepted policy change and does not satisfy the
current GH-239 completion requirement.

1. In GH-239, finish source changes through reviewed PRs, then rebuild the final
   candidate once, collect clean-install/native/Core/lifecycle evidence and
   complete the actual per-payload license decisions. Generate local manifests
   containing every evidence filename, size and SHA-256, including required source
   and build inputs and the bootstrap. Retain a hash-verified second copy.
2. Present the exact preflight inventory/source/tag and repository-wide immutable
   release setting change for explicit approval under GH-240. Describe the impact
   on future Core releases: published assets become locked. No API setting write
   is authorized by this handoff.
3. After that approval, create the source-bound
   `rust-collector-v0.1.0-rc.1-evidence-preflight` draft, upload the complete approved
   inventory, independently download and compare all bytes, then publish and
   verify immutability and stable HTTPS downloads. Record asset IDs and recovery
   verification. Do not use expiring Actions artifact URLs as permanent evidence.
4. Bind those actual URLs and retained receipts into the final packet, revalidate
   the packet and exact-source main CI, then obtain the separate concrete
   collector publication approval required by GH-240. Only then execute the
   reviewed protected six-asset collector workflow.
5. Publish the separately approved
   `rust-collector-v0.1.0-rc.1-evidence-published` inventory for final download,
   approval and teardown receipts. Include its exact local inventory in its
   approval; do not append assets to the already locked preflight release.

The proposed retention owner remains higoalespn, with supported-lifetime retention,
no automatic deletion and recoverable local backups afterward. Actual owner
acceptance, repository setting readback and published immutable evidence receipts
are still required. Planned names are not live URLs. Whole-release/repository
loss is addressed by the verified second copy, not by claiming GitHub is WORM.

## Completion criteria

Do not mark P8.1 complete from this preparation PR. The real runner/signing
context, final license decisions, accepted publication ordering, final rebuilt
candidate and immutable evidence are outstanding. GH-240 still requires both
explicit packet-bound publication approval and the protected environment review.
