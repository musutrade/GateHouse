# GH-231 reopened release preparation

> Storage update (2026-10-13): historical raw logs/bundles are available through
> the [verified archive guide](archive.md). Dates, digests and outcomes below are unchanged.

Preparation implementation is ready for review; publication approval is **blocked**.
No tag, production signature or release was created. P8.1a–f remain unchecked
where their full acceptance depends on the inputs below; P8.2/P8.3 and GH-215
remain open. This record supersedes the earlier status, while preserving the
[PR #232 validation history](validation.md).

## Source and bounded implementation

The assigned branch started at reviewed main
`48099c48f1fded35af5d71638bf57160cb8e0b31`, containing accepted
[PR #237](https://github.com/musutrade/Harness-Gate/pull/237). The initial API
refresh confirmed the predecessor merged and origin/main at that SHA.
[Main CI 34601694875](https://github.com/musutrade/Harness-Gate/actions/runs/34601694875)
was still in progress with no successful Required Quality Aggregate receipt at
that refresh. This is not successful main CI evidence. The controller owns
subsequent CI and acceptance. The collector environment GET returned 404;
the existing Core environment was not changed.

The six bounded P8.1a–f units are recorded in the accepted change's
[tasks](../../../openspec/changes/package-official-rust-collector-for-independent-delivery/tasks.md).
This PR prepares the following reviewable parts of those units:

- An independently provisioned shell launcher that checks a private copy of the
  standalone installer capsule before extraction or execution. The capsule
  includes private Python, installer modules, schemas and dependency notices.
- Production trust v2, requiring both the existing pinned RSA/SHA-256 signature
  and an offline Sigstore bundle over identical inventory bytes. A versioned
  envelope occupies `release-inventory.sig`, preserving exactly six RC assets.
- Unsigned candidate preparation that validates the complete tar payload and
  emits a descriptive SBOM and an explicitly ineligible receipt.
- A protected production workflow **template**, outside `.github/workflows`,
  and helpers that rebuild pinned inputs, compare approved bytes, require exact
  main CI, environment protection, license closure, final matrix and pinned
  validation receipts, then verify the actual independent tag before signing.

The current `.github/workflows/rust-collector-release.yml` remains a read-only
synthetic rehearsal. Installing the template at that exact path requires review
and provisioning; a template test or merged preparation PR is not production
readiness. The source tree's Git metadata is sandbox read-only: the initial
`git fetch --no-tags origin main` failed opening `.git/FETCH_HEAD`. Remote API
reads confirmed the baseline; delivery uses the same remote issue branch without
altering local Git metadata or accessing another checkout. The submission
preserves the old issue head as a second parent; its only changed file is already
in main byte-for-byte ([reconciliation receipt](https://github.com/musutrade/Harness-Gate/blob/d9a932b23dcfea709175285b594ca200450a9971/docs/quality/gh-231/evidence/branch-reconciliation.json)).

## Actual candidate bytes and dependency audit

The new runtime was bootstrapped from the operator-provisioned official archive,
using a new workspace-local compiler overlay and build directory. It does not
reuse GH-230 captures or its archive. The candidate source is the reviewed SHA
above and its manifest version is `0.1.0-rc.1`; it is **unsigned and ineligible**.
The [machine receipt](https://github.com/musutrade/Harness-Gate/blob/d9a932b23dcfea709175285b594ca200450a9971/docs/quality/gh-231/preparation-receipt.json) records the actual observed ABI,
tool identities, Core identity, crate notices and OS package closure.

| Local asset under `target/gh-231/candidate/` | Bytes | SHA-256 |
| --- | ---: | --- |
| `collector.tar` | 1623490560 | `3143043f3093a97a6aaebcaac964eb27f9db2f41d9e013cad92a514b9ba18bfa` |
| `manifest.json` | 213560 | `ee184213fbf96ccb8781a6fa0eafdd7a3512c40321a80917ba8a85e8bf1158a0` |
| `sbom.spdx.json` | 482318 | `24a9bc4fe35a04197ae8dff96034158de36ab31b0462d792d1c1e51147f89ba9` |

The independently distributed installer is outside the six collector assets:
`target/gh-231/installer-bootstrap.tar`, 53780480 bytes, SHA-256
`e5e2029ec7b33493ca2ef05e89eba3630c40a4b4c4dd9ed53958d0fbf1a72408`.
Its 687 inventoried files include the reviewed-source candidate installer.
The launcher SHA is in the machine receipt. The
[host-input receipt](https://github.com/musutrade/Harness-Gate/blob/d9a932b23dcfea709175285b594ca200450a9971/docs/quality/gh-231/bootstrap-host-inputs.json) records the actual shell, tar,
core utilities and loader/library hashes; observation does not authenticate them
or establish fresh-host acceptance. These are descriptive build hashes,
not an authenticated public trust channel or controller approval.

The archive has 1102 verified payloads, including 82 license/notice payloads,
21 checksum-pinned original crate archives with license declarations/notices,
and 24 OS package identities. The SBOM records file hashes with `NOASSERTION`
license conclusions. Notices and source archives are present, but per-payload
redistribution decisions, toolchain/OS source-offer obligations and complete
production license approval remain unreviewed. The proposed workflow rejects
missing or duplicate payload decisions and `NOASSERTION`/pending approval.

These artifacts are local only. No durable artifact URL, retention owner or
retention policy has been provisioned. Preserve the entire `target/gh-231/`
tree before workspace deletion; Git receipts cannot recover the large archive.
The eventual reviewed production workflow will live at a newer source commit:
rebuild and reapprove that exact source/manifest/matrix before publication.
Do not relabel this source receipt or retain this manifest digest for that build.
Production provenance, final inventory and dual signatures are not yet produced.

## Installation and distribution trust

The bootstrap requires an administrator-authenticated OS shell, tar, coreutils,
loader and exact supported host libraries. It removes ambient Python/loader
overrides, copies the capsule privately, hashes that copy, then extracts and
invokes its private Python with `-I -S -B`. There is no host Python or source
checkout dependency in that entry path. The administrator must independently
authenticate **both** the launcher and capsule SHA before use. A SHA downloaded
beside an unauthenticated capsule does not establish trust.

The actual capsule's private Python loaded the installer and printed help from
a separate working directory, and rejected v1 trust. Authentication tests also
rejected missing, empty/untrusted-digest capsules before a sentinel executable
could run. These prove entry-path behavior, not a complete clean-host install.
Docker denied access to `/var/run/docker.sock`; bubblewrap denied creation of a
user namespace. No fresh host without Python/source checkout was available for
the required production positive. These failures are retained.

Provision a JSON `rust-collector-host-trust/v2` outside the release directory.
It retains v1's absolute `openssl`, `public_key`, their `_sha256` pins,
`rsa_signature_bytes` and `host_libraries`, and adds absolute `cosign` and
`trusted_root` paths with their `_sha256` pins. Authenticate the actual verifier
version, trusted-root contents, dynamic dependencies, RSA key and host libraries
through the administrator channel. The artifact cannot provide its own verifier.

The proposed Sigstore verifier uses `verify-blob --offline --bundle ...
--trusted-root ...` with exact certificate identity
`https://github.com/musutrade/Harness-Gate/.github/workflows/rust-collector-release.yml@refs/heads/main`
and issuer `https://token.actions.githubusercontent.com`. It supplies no tlog
bypass, regular-expression identity or ambient Sigstore configuration.
See the official [verification CLI](https://github.com/sigstore/cosign/blob/main/doc/cosign_verify-blob.md)
and [trusted-root configuration](https://docs.sigstore.dev/cosign/system_config/custom_components/).
The identity authenticates the protected main workflow; the signed inventory
and provenance independently bind the collector tag, version and exact source.

No pinned cosign executable/root or production OIDC signing context was supplied.
Tests use real local RSA and a clearly labeled cosign subprocess double. They
exercise missing/extra/tampered/unsigned assets, missing bundle, verifier failure
(including wrong identity or inclusion proof rejection), stale receipts and
changed host verifier. They do not establish a real Sigstore positive or real
wrong-certificate/inclusion negative. Those receipts remain mandatory.

After independently authenticated provisioning, the intended invocation is:

```sh
sh /administrator/bootstrap_collector.sh "$TRUSTED_BOOTSTRAP_SHA256" \
  /administrator/installer-bootstrap.tar --trust /administrator/trust-v2.json \
  --root /opt/harness-gate-rust install --release /downloaded/six-assets \
  --tag rust-collector-v0.1.0-rc.1
```

Use the installer's actual `--help` for lifecycle syntax. Distribution verification
does not authenticate runtime captures. Package version/path relocation does not
establish series equivalence. Core 0.4.0 remains the policy authority.

## Validation and compatibility limits

[Command receipts](commands.md) give exact commands, counts and retained failure
logs. All 392 Core tests passed with proxy variables removed after the original
localhost webhook failure. Full quality tests passed (412, three opt-in native
classes skipped); separate native runs cover the configured runtime and compiler.
The runtime suite passed all 11 tests after correcting its fixture vendor input
and draining stdin in the intentionally malformed producer. No policy semantics
or business tests changed.
All 54 release tests, format, clippy, documentation consistency and strict
OpenSpec validation passed; original failed attempts remain in the evidence.

The native runtime tests exercise fresh host-built runtime bytes and released
Core via the generic invocation, lifecycle and negative paths. Their fixture
package/trust identities are test-only. The RC manifest additionally uses a new
RC binding derived from fresh local native evidence; it does not convert those
fixture results into production RC certification. Archive validation proved
actual RC payload/hash correspondence without executing unauthenticated assets.

The actual observed tuple is Linux x86_64, glibc 2.43, kernel
`7.0.0-31-generic`, exact pinned library/tool identities, and released Core 0.4.0
SHA `8e3df8303ca8f650d4ef768b29cfefb60ca115122a47b116245cc19e4649bbdd`.
Scope remains `single-file-fixture`. The production matrix still has `tested: []`;
no support row, broad Linux claim, baseline acceptance or stable promotion is
authorized by this preparation.

## Concrete operator approval packet and publication sequence

The receipt here has `blocked-not-publication-approval` status. The following
operator-owned prerequisites must be supplied before a usable
`rust-collector-publication-approval/v1` packet can be reviewed:

1. Review and install [the production workflow template](https://github.com/musutrade/GateHouse/blob/da8e8a58a4ac5959dc0346b79bad4a4437b964cb/tools/release/rust-collector-release.production.yml)
   at the exact identity path. Provision `rust-collector-release` with nonempty
   required reviewers, `prevent_self_review: true`, `can_admins_bypass: false`,
   and protected-main deployment restrictions. Supply actual reviewer IDs;
   do not reuse/weaken the Core `release` environment. Use an ephemeral runner
   labeled `rust-collector-release` matching the approved ABI. The protected job
   alone needs contents-write and OIDC-write permissions.
2. Provision pinned build inputs and the independently trusted bootstrap, launcher,
   OpenSSL/RSA public key, cosign and trusted root. Store the corresponding RSA
   private key only as protected environment secret `COLLECTOR_RSA_PRIVATE_KEY`.
   Set environment variable `COLLECTOR_APPROVAL_PACKET` to the absolute reviewed
   packet path. No private key is generated or retained by preparation.
3. Rebuild the final reviewed main source. Supply `source_commit`, version
   `0.1.0-rc.1`, and `inputs` entries each containing absolute `path` and `sha256`
   for `manifest`, `build_lock`, `bootstrap`, `bootstrap_launcher`, `trust`,
   `compatibility`, `observed_environment`, `license_review`, and `validation`.
   `unsigned_assets` must map exactly `collector.tar`, `manifest.json` and
   `sbom.spdx.json` to actual `sha256` and `size`. Approval of the packet SHA is
   independent of the build job. No successful eligibility receipt is needed or
   fabricated to stage unsigned assets.
4. Supply the exact reviewed matrix row and its pinned native receipt, production
   license review with reviewer, exact manifest SHA and every payload's
   path/SHA/license/approved redistribution. Supply validation with exact source
   and manifest SHA plus `checks` for `fresh_host_install`,
   `native_capture_reexport`, `generic_core`, `lifecycle_negatives`,
   `bootstrap_negatives`, and `real_sigstore_positive_and_negatives`. Every check
   requires `status: pass`, an immutable HTTPS `receipt_url`, local `path`, and
   verified `sha256`. These are actual reviewed evidence, not generated assertions.
5. Set `retention.immutable_url`, `retention.owner` and `retention.policy` for the
   actual artifacts, bootstrap and receipts. Successful exact-source main CI and
   Required Quality Aggregate remain mandatory. Obtain explicit controller
   publication approval and separate protected-environment approval.

The template contains the exact planned commands: `assemble` verifies all inputs
and compares unsigned bytes; `gh api --method POST .../git/refs` creates only
`rust-collector-v0.1.0-rc.1` at the approved source; `sign` rechecks real tag
eligibility and signs the inventory with protected RSA plus Sigstore; `gh release
create --verify-tag --draft --prerelease` uploads exactly six assets. A fresh
download is verified before making the draft public, then a second fresh download
is verified after publication. Each download must match all six original signed
files byte-for-byte and the independently approved unsigned digests; verification
emits a fresh receipt outside the six-asset directory. Retain both verification receipts and actual final
inventory/provenance/signature digests. The workflow's 90-day artifact upload is
supplemental and cannot substitute for the approved durable retention policy.

If assembly/signing/draft verification fails, stop and retain diagnostic evidence.
Never move/reuse the immutable tag or overwrite assets; a tag created before a
failure requires operator reconciliation. If public verification fails, stop
handoff, notify the controller and quarantine the release through the approved
operator process. Fix with a separately approved new version, not replacement
bytes. Installed hosts may select a previously verified version using the
installer's rollback operation; preserve capture identity and never reset history.

The Arc-Admin owner handoff after publication must contain the immutable version,
source, all six digests and sizes, bootstrap trust channel, installation and
download verification receipts, exact compatibility/ABI and retention owner/URL.
Until then it is pending. GH-215 stays independently open and stable promotion
requires separately accepted fresh Arc-Admin integration. No cross-repository
write, threshold change, baseline acceptance or gate transfer is included.
