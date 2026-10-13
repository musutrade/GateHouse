# GH-239 operator preparation and proposed personal release approval

The owner confirmed this is a personal project with no second reviewer. This PR
proposes a **delivery-governance delta** to GH-229's mandatory separation of
trigger and reviewer: only the dedicated collector environment may permit its
single pinned owner (`higoalespn`, GitHub user ID `23396976`) to approve a run they
triggered. This is an explicit exception, not a claim of independent human review.
Environment configuration has **not** been applied; publication remains unauthorized.

The proposed environment has exactly that required User reviewer,
`prevent_self_review: false`, `can_admins_bypass: false`, and a custom deployment
branch policy allowing only `main` (no tag rule). Manual environment approval,
exact-source main CI and Required Quality Aggregate, manifest/digest-bound approval,
RSA plus Sigstore verification, inventory/provenance, and download verification
remain required. Existing Core release settings are unchanged. All relevant
Engineering Policy quality, baseline/ratchet, fail-closed and authority semantics
remain unchanged; this exception changes only the collector delivery separation
rule. No stable promotion, baseline acceptance or Arc-Admin gate transfer.

The collector eligibility receipt uses `rust-collector-eligibility/v2` for this
personal mode and binds the owner identity and actual self-review setting. V1
continues to mean the previous separate-reviewer policy. An incorrect owner,
extra reviewer, wrong environment, bypass, missing review gate or a v1 receipt
claiming the personal exception fails. Old installers fail closed on v2 receipts;
rebuild the final bootstrap from the reviewed new source before publishing.
Changing the owner requires a reviewed source change. Removing this exception
requires re-enabling separate review and using the v1 mode for future releases;
retain historical v2 receipts and never rewrite release bytes or evidence.

## Provisioned and observed inputs

Operator source baseline: accepted PR #238 / main
`edebce46ccb69a1cabc0507ed8bbcce6d4b74c9d`. A separate clean Symphony workspace at
`/mnt/dev-ssd/workspaces/symphony/GH-239` contains that source, the independently
verified official rustc-dev input, and released Core 0.4.0 with retained release
verification metadata. It is not yet queued for implementation.

The GH-230-only runner is insufficient for this issue. A dedicated operator-owned
`/home/gem/.local/bin/symphony-gh239-container.py` and
`symphony-gh239-container.service` now accept only GH-239-local runtime/output
paths. The fixed image is
`sha256:513c074113a871b51a8d16ab445c88779d6452d937a164fb5cc479f32668a41d`.
Each execution uses no network, read-only root/runtime, no capabilities,
no-new-privileges, uid 1000, and bounded processes/memory/CPU. No Docker socket or
other source/workspace is mounted. Four boundary negatives rejected an external
output directory, overlapping mounts, an invalid entrypoint and image override.

`/home/gem/.local/bin/symphony-gh239-preflight.py` was tested with the actual
configured Codex app-server `command/exec` sandbox policy and added as a GH-239-only
before-run check. The isolation probe and real offline Sigstore verification both
passed. Receipts are under
`target/gh-239/operator-acceptance/sandbox-preflight-20260911T144113554178Z/` in the
assigned workspace; host-side receipts persist under
`/home/gem/.local/state/symphony/gh239-provisioning` and
`/home/gem/.local/state/symphony/gh239-sandbox-recovery/container-runs`.
The probe observed kernel `7.0.0-31-generic`, glibc 2.43, no ambient Python,
no source checkout, no Docker socket and an unwritable runtime. This supplies a
clean container execution interface, not a new VM or production GitHub runner.

The historical GH-231 bootstrap, independently pinned by its retained SHA,
entered the private-Python installer in this container and then rejected the
missing mandatory `--trust` argument (exit 2). This is an expected entry-path
negative, not successful RC installation. Original logs are retained. Final
source-bound capsule/RC installation and all native acceptance remain required.

## Real verification tool bootstrap

Cosign **v3.1.3**, binary SHA-256
`4629c757b7618056f8ddd7e2625ae9fdd94c0372a65049520bc7d9df9efc7f71`, was downloaded
from the official release and checked against GitHub's recorded asset digest.
Before execution, its KMS signature was verified with host OpenSSL using the
artifact public key obtained by a TUF refresh bootstrapped from Sigstore root 10.
TUF 6.0.0 and securesystemslib 1.3.1 were extracted from authenticated Ubuntu
packages into an isolated operator tools directory; no global Python change.
See the [upstream bootstrap procedure](https://docs.sigstore.dev/cosign/system_config/installation/).

After that independent verification, the actual cosign binary verified the
upstream real identity bundle offline, including inclusion verification. Four
real negatives rejected wrong identity, missing bundle, tampered signature and
missing inclusion material. The same offline positive passed inside the fixed
container through the Codex sandbox. The verified trusted-root file SHA-256 is
`6494e21ea73fa7ee769f85f57d5a3e6a08725eae1e38c755fc3517c9e6bc0b66`.

Inputs and the pinned provisioning manifest reside at
`/mnt/dev-ssd/symphony-inputs/gh239/cosign-v3.1.3`; copies are available under
`target/symphony-inputs/cosign-v3.1.3` and are SHA-verified before staging in the
container. These verify the upstream cosign identity
`keyless@projectsigstore.iam.gserviceaccount.com` / `https://accounts.google.com`.
They do **not** certify the Harness-Gate production workflow identity, sign a
collector candidate, provision its production RSA key or satisfy final RC trust.

## Remaining decisions and work

- Review/approve the single-maintainer exception and apply the exact environment
  plus main-only branch rule. No environment mutation has occurred in this PR.
- Provision protected production RSA trust, a suitable ephemeral GitHub release
  runner and the separately approved real collector signing context.
- Complete actual license/redistribution review and choose the immutable HTTPS
  artifact destination/retention policy. Local historical backup is not enough.
- Activate the production workflow through review, rebuild final source-bound
  candidate/bootstrap, complete clean-host/native/Core/lifecycle acceptance and
  pin the final compatibility and approval packet.
- GH-240 alone owns separately approved publication and the two download checks.
  GH-239, GH-225 and GH-215 remain open. This PR must not close GH-239.


## Local validation of this proposed delta

[Retained evidence](https://github.com/musutrade/Harness-Gate/blob/d9a932b23dcfea709175285b594ca200450a9971/docs/quality/gh-239/evidence/index.json) records actual operator checks and
proposed environment/main-branch JSON payloads. `python3 -m unittest discover
-s tools/release/tests -v` passed **55 tests** after correcting the new test's
fixture version from a non-release placeholder to `0.1.0-rc.1`; the original
failed attempt is retained. This suite covers the real RSA installer/release
path plus the personal-mode eligibility/receipt acceptance and rejection cases.
No required test was removed or bypassed. Documentation consistency,
telemetry-disabled strict OpenSpec validation and `git diff --check` passed.
No Rust code changed; the unchanged Rust suites were not rerun locally.
Hosted required CI and policy review remain pending. No Symphony completion
handoff is written: this PR intentionally leaves GH-239 acceptance unfinished.
