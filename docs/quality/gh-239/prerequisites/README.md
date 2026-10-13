# GH-239 release prerequisites

Status: runner-group access provisioned; execution and release approval pending.
GH-239 remains open and unready. Relevant Engineering Policy semantics are
unchanged. No publication, signature, tag, baseline or Arc-Admin transfer is
approved by this document.

## Current state and ownership

The 2026-09-11 API refresh observed zero repository Actions runners. The dedicated
collector environment has reviewer higoalespn (23396976), self-review enabled,
admin bypass disabled. These previously approved settings are retained. Main
74a410759baed25983d6647c70cc99cb1e71cafc has CI run 34614379129; it was still in
progress at inspection. Historical preparation documents retain their original
observations; this document does not turn them into final acceptance.

The proposed operator, license decision maker and retention owner is the sole
maintainer higoalespn. This is an assignment proposal, not a fabricated license
approval. The owner selected GitHub storage in the 2026-09-11 conversation; no external bucket or domain is needed.

## Production runner configuration

### Provisioned runner-group access boundary

The initial organization inspection exposed no dedicated collector group.
The scoped create request in `runner-group.request.json` was submitted on
2026-09-11 and rejected with HTTP 403, `Resource not accessible by personal access
token`. The actual response is retained in `runner-group.create-failure.json`.
That initial attempt created no group or runner. Repository admin permission does not establish
organization runner-management permission. An organization owner must execute the
request using an appropriately scoped credential (organization self-hosted runners
write permission) or create the identical group through GitHub settings. Do not
send a token in a PR, issue, chat or evidence file.

The owner subsequently updated the credential permissions. Retry succeeded:
organization runner group **4**, `harness-gate-rust-collector-release`, is now
provisioned. Independent GET readback verified exactly repository ID 1346842829
and exactly the main collector workflow, selected visibility, and enforced
workflow restrictions. `runner-group-readback.json`,
`runner-group-repositories.json` and `runner-group-runners.json` retain the
responses. The group has zero runners: this establishes scheduling access only,
not production host, runner execution, signing or RC acceptance. The earlier 403
remains retained history. Do not recreate group 4 or request the same permission
again. The following create command is historical/recovery reference only.

```sh
gh api --method POST orgs/musutrade/actions/runner-groups \
  --input docs/quality/gh-239/prerequisites/runner-group.request.json
```

First re-list groups to avoid duplicate creation on retry. Read back the created
group and selected repositories: require exactly repository ID 1346842829,
`visibility=selected`, `restricted_to_workflows=true`, and exactly the main
collector workflow in the request. The public-repository allowance is necessary
for this one selected public repository; it is not all-repository access. Register
at organization scope with `--runnergroup harness-gate-rust-collector-release`,
not as a repository-scoped runner outside the group. The future production job
selector must use `group: harness-gate-rust-collector-release` and
`labels: rust-collector-release` together.

The runner download API currently supplies Linux x64 runner 2.337.0 with SHA-256
70920811a4f8ad4328818682bca5c6469c1c942fab52448868071d0063816613.
That initial metadata-only observation is superseded by the verified archive and
local startup receipts in [runner preparation](runner/README.md).
The local host has /dev/kvm, but no qemu-system-x86_64 or virsh executable was
found and the current user is not in the kvm group. No disposable VM or registered production
runner has been provisioned. A separate constrained runner image has now passed
local offline startup/isolation checks; the existing GH-239 acceptance service
remains unchanged. See [actual receipts](runner/README.md).



Provision a dedicated disposable host, with no mounts of the developer home,
Symphony workspaces, Docker socket or other credentials. Do not register the
existing developer host as a general repository runner. A one-job runner is
not by itself workload isolation. The clean-install container already provisioned
for GH-239 is a separate acceptance interface, not an Actions runner.

Use Linux x86_64 with the exact approved host-library hashes and kernel. The
historical tuple is glibc 2.43 / kernel 7.0.0-31-generic / dependency fingerprint
f9b213877a2ea9aa44e6119e6fe2e1361f84f9a8084abd410ab8ef34b74e2872.
A host with only matching version strings is insufficient. Re-probe against the
final manifest before accepting any row. No broader compatibility is implied.

Download the runner using the repository's Actions runner download API, retain
its version/URL/SHA-256 and verify the archive before execution. Register only
when the reviewed main publication job is approved and ready, using a short-lived
registration token, --ephemeral --disableupdate --no-default-labels and label
rust-collector-release. Use the group-and-label selector above in the reviewed production template
when using no-default-labels. Never claim a label is an
authorization boundary. Before registration, inspect all queued and in-progress
jobs and workflow selectors, refuse any competing/untrusted job, and restrict
runner access to the approved workflow through runner-group policy where the
account supports it. If enforceable scheduling isolation is unavailable, keep
provisioning blocked; do not rely solely on a quiet queue.

Stage approved inputs at operator-owned read-only absolute paths; the packet
must refer to those actual paths and hashes. Give the runner only its disposable
work directory. Keep the production RSA key solely in the existing protected
environment secret. Forward runner diagnostics to durable storage before host
teardown. After one job, confirm deregistration and destroy the disposable host.
A retry requires a fresh host and fresh provenance. Pin and revalidate the runner
version for each execution; disabling updates does not waive GitHub's minimum
supported-version requirement.

Acceptance receipts must include host/image identity, full ABI probe, runner
version/digest/id, enforced workflow access policy, job/run/source identity,
input digest verification, external log receipt and deregistration/teardown.
No production dispatch or OIDC signature test occurs under this preparation;
GH-240 owns the separately approved production execution context.

References: [GitHub runner reference](https://docs.github.com/en/actions/reference/runners/self-hosted-runners)
and [security guidance](https://docs.github.com/en/enterprise-cloud%40latest/actions/reference/security/secure-use).

## License review worklist

`historical-payload-review.json` is an actual hash-verified inventory of the
retained GH-231 archive, not the final RC license-review input. Each row starts
pending; do not bulk-convert it to approved. It records notice-file hashes but
does not assume a notice applies to every binary. Review the standalone bootstrap
separately as well as the collector. The final reviewer must bind decisions to
all final manifest payload paths and SHA-256 values, with evidence of applicable
license, copyright, notices and fulfilled redistribution obligations.

| Payload family | Evidence needed before approval |
| --- | --- |
| Harness-Gate code | Source-bound MIT license and copyright notice |
| Rust compiler, rustc-dev, Cargo, LLVM | Exact distribution license inventory, bundled third-party notices and source provenance |
| Vendored crates | Original archive checksum, applicable license expression, license texts and notices; preserve Unicode terms where applicable |
| Python and extension modules | Exact CPython and embedded third-party terms, package source/version mapping |
| OS libraries and compiler/linker tools | Per-file package ownership and exact corresponding source; assess GPL/LGPL terms and any exceptions per payload |
| Installer capsule and launcher | Complete independent payload inventory and the same review, including private Python/libraries |

For source obligations, retain the exact corresponding source and build/patch
materials required by the applicable terms, linked by hashes in the retained
bundle. A generic upstream homepage or binary-package copyright file alone is
not evidence that an obligation is fulfilled. Unmapped payloads block approval.
This is a technical audit worklist; the named human reviewer must make the actual
redistribution decisions. Rebuild the final candidate before producing
`license_review.status=approved`; the historical manifest cannot approve it.

## Selected retention destination: GitHub

The owner selected GitHub on 2026-09-11. Use GitHub Releases for collector binary
assets and separate versioned evidence releases for bootstrap, build inputs,
license/source materials and verification receipts. crates.io remains the Rust
crate channel; this collector distribution includes a compiler/runtime and
standalone installer and is not a substitute crate publication.

Proposed retention owner is higoalespn. Preserve releases for the supported
lifetime and retain recoverable local backups afterward; no automatic deletion.
This is an owner-operated retention policy, not a contractual WORM guarantee.
GitHub immutable releases prevent individual asset replacement/deletion and tag
movement after publication, but whole-release/repository deletion remains a risk.
The existing backup at /mnt/dev-ssd/artifacts/harness-gate remains a second copy;
verify it against the final GitHub downloads and periodically exercise recovery.

The repository API reports immutable releases disabled. Prepare the repository-wide
setting change for review, including future Core-release impact; do not silently
change Core governance. GitHub recommends preparing a draft with all assets, then
publishing it to lock the complete set. No setting or release was changed here.

Keep exactly six assets on rust-collector-v0.1.0-rc.1. Use separate evidence
releases so bootstrap/build inputs do not alter that contract. Suggested immutable
names (not created): rust-collector-v0.1.0-rc.1-evidence-preflight for prepublication
inputs and rust-collector-v0.1.0-rc.1-evidence-published for final download/approval
receipts. Bind their tags to reviewed source through GH-240's explicit approval;
add their exact inventories and commands to that packet before execution.
Never append post-publication receipts to an already immutable release.

Expected HTTPS form:
https://github.com/musutrade/Harness-Gate/releases/download/<evidence-tag>/<asset>.
These are planned locations, not existing receipt URLs. Record asset IDs, byte
sizes, SHA-256, source/tag and immutable-release attestations after upload.
Verify all downloads with the independent trust chain and compare original bytes.
Do not use Actions artifacts or expiring redirect URLs as permanent references.
Retain large source/build inputs as deterministic, hashed parts if a file exceeds
GitHub's current per-asset limit; record part order and reassembly hash and verify
reassembly. Preserve bootstrap authentication separately from its download URL.

There is an unresolved ordering requirement: current GH-239 validation demands
immutable HTTPS evidence before the final approval packet, while GH-239 forbids
creating tags/releases. An unpublished GitHub draft cannot satisfy immutability.
Prepare all source-bound evidence locally in GH-239, then explicitly include the
preflight evidence publication in GH-240's concrete approval scope, or propose a
reviewed preparation-only publication exception. Until this boundary is resolved,
keep the final packet blocked; do not invent live URLs or mark P8.1 complete.

Acceptance requires immutable-setting readback, published asset/attestation
receipts, independent HTTPS digest verification and a verified second-copy
recovery receipt. No external storage account, domain or seven-year paid
compliance-lock configuration is requested.

Reference: [GitHub immutable releases](https://docs.github.com/en/code-security/concepts/supply-chain-security/immutable-releases).

## Next acceptance boundary

Supply an isolated runner host/access policy and resolve GitHub evidence publication ordering;
complete the named review against final rebuilt bytes. Activate the production
workflow through a normal PR only after those provisions are usable. Rebuild
from the resulting final main, wait for that source's required CI, and complete
clean install/native/Core/lifecycle acceptance. Keep GH-239 open until the
concrete final packet is verified, and GH-240 separately publication-gated.

## Follow-up inspection

[Historical license materials](license-materials-review.md) now cover both the
collector and standalone bootstrap archive, with per-file digest verification
and embedded crate declarations/text hashes. No license decision is inferred.
[Execution handoff](execution-handoff.md) specifies pending real-job receipts and
a proposed two-phase GitHub evidence publication ordering for GH-240 review.
