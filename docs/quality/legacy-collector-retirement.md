# Legacy bundled collector workflow retirement

On 2026-10-14, GateHouse issue #12 retires the paused six-asset RSA/Sigstore
bundled-runtime entry from Core's active workflows. The product remains paused;
GateProbes owns any future revival. This is separate from the independently
published native Rust/MIR RC8 external-toolchain product.

Engineering Policy semantics are unchanged: collectors measure facts; Rust Core
retains requiredness, thresholds, lineage, baseline/debt/ratchet and final release
authority. No publication, credential provisioning or baseline adoption occurs.

## Exact source and disposition

| Field | Value |
| --- | --- |
| Former entry | `.github/workflows/rust-collector-release.yml` |
| Archive | [`retired/workflows/rust-collector-release.yml`](../../retired/workflows/rust-collector-release.yml) |
| Restore source commit | `189fc31c4f12167b0737eff369d64aba700ab5a0` in GateHouse |
| Original import commit | `7f1fdac4b74b6bbf2ccb20cd4499d4ab4d497af5` in GateHouse |
| SHA-256 of original and archived bytes | `76e2b92297d5e22e3a656e2cf9acdc838f89dabc6094e7a9e3eaa986d3f00d15` |
| Disposition | `retired-restorable`; publication state `paused-not-migrated` |
| Archive owner / future revival owner | GateHouse / GateProbes |

The YAML is moved unchanged, including its historical Harness-Gate identity,
repository/ref restrictions, migration hold and protected environment requirements.
Current Core branches containing this change have no such active entry. Historical
commits still contain their original workflows; existing tags, releases,
signatures and dated acceptance/failure records are not changed.

## Consumers retained

| Consumer | Disposition |
| --- | --- |
| `test_release_ownership.py` | Reads the retired ownership row, requires an absent active entry, and checks the archive digest and revival owner. |
| `test_stable_collector_policy.py` | Executes the archived hold: rehearsal succeeds; signing, publication and installer publication each reject before environment access. All mutation jobs must depend on protection. |
| `test_collector_delivery.py` | Reads the archive for protected rehearsal/signing/publication contracts. Environment approval and unsigned/tampered/wrong-identity negatives remain. |
| `tools/release/collector_assets.py` | Keeps the original workflow identity for historical signature verification; archive location does not rewrite a signed identity. |
| Legacy packaging, signing, installer and publication scripts/templates | Retained with their tests and GateProbes ownership. Physical removal requires the separate #13 consumer review. The production template is not an active workflow or authority to revive the product. |
| `.github/workflows/release.yml` and `install.sh` | Retain Core publication and explicit plugin installation; neither invokes the retired entry. Their bytes remain unchanged by this retirement. |

Ordinary CI and Core release continue to discover `tools/release/tests`.
The archive replaces a test input; no missing-file skip or coverage exclusion is
introduced. Production-policy tests retain rejected approval, source, CI,
signature and payload cases. Synthetic tests do not establish actual signing or
product acceptance.

## Rollback and future product decisions

For source recovery, read the former path at the exact restore source commit
above, or use the byte-identical archive. Verify its SHA-256 before use. A reviewed
rollback may move those exact bytes back and revert the corresponding ownership
and test-path changes together. It must retain the repository/ref restrictions,
migration hold, protection dependencies, protected environment and packet checks;
it must not add credentials, provision runners, change trust identities or
enable publication. Restoring the file leaves all mutating selections rejected.

Any future product revival must be implemented by GateProbes after an explicit
product decision and a reviewed measurement migration satisfying Engineering
Policy section 10. It needs its own identity, approval and acceptance review.
Copying this archive or the old production template is not that approval, and
the native RC8 release does not waive the bundled-product hold.
