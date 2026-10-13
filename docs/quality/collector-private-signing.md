# Private RC signing before installation acceptance

The former preparation flow required successful production-signature and fresh
installation receipts before the first production signature could exist. The
manual collector workflow now accepts an optional `signing_packet_sha256` input.
With neither digest input, the protected synthetic rehearsal remains the default.
The separate `publication_packet_sha256` input selects final publication using
`COLLECTOR_APPROVAL_PACKET`; selecting both operations is rejected. Both entry
points live in the same source commit so enabling publication does not invalidate
the tag and manifest that were already accepted.

With it, the existing main-only workflow and restricted local runner group run a
separate job behind the existing `rust-collector-release` environment approval.
The host-owned `COLLECTOR_SIGNING_PACKET` variable points to an absolute JSON file
whose exact SHA-256 is supplied by the operator. Its schema is
`rust-collector-private-signing/v1`; it pins the source commit, RC version,
unsigned three-asset directory and hashes/sizes, independent host trust file and
hash, and a fresh absolute output directory.

Before accessing the RSA key, preflight verifies exact checked-out main, actual
main CI, protected environment, manifest identity, every archive payload, SBOM,
pinned verifier inputs and host ABI. Signing repeats preflight, creates a new
immutable RC tag (an existing tag stops this stage), verifies actual tag
eligibility, and produces the existing six-asset RSA/Sigstore delivery format.
The signed candidate stays on the host. No GitHub Release is created and no
candidate assets are uploaded by this job. A receipt explicitly records that
installation acceptance and publication have not happened. A tag is public;
it does not publish the binary assets or assert final project acceptance.

These bytes allow actual production-trust installation and signature negatives
to run before public distribution. The separately selected publication job still requires the unchanged
license approval, reviewed compatibility tuple, all six successful validation
receipts, exact unsigned bytes and independently downloaded asset verification.
The production template may reuse the exact commit tag created for acceptance;
it rejects a different target and never updates an existing tag. Existing
GitHub Releases are never overwritten. An interrupted private signing attempt
that already created a tag requires a new RC version.

The workflow change does not modify Core quality thresholds, debt baselines or
project workflow state. Real environment approval and the signing run remain
operator actions; local tests use explicit doubles for GitHub and Sigstore and
must not be reported as actual production signing.
