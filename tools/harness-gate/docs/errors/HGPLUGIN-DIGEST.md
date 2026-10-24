# HGPLUGIN-DIGEST

## Why this fails

The prepared manifest bytes do not match `pin.manifest_sha256` from the lock, or a copied artifact does not match the digest recorded for that path in the signed manifest's `files` inventory. The lock pins the manifest digest; artifact digests come from the signed manifest, not directly from the lock.

## Diagnose and repair

Restore the exact pinned manifest named by the lock. For an artifact mismatch, restore the exact signed file declared in the manifest inventory. Do not edit the lock or the signed inventory to hide corruption; review and explicitly lock any upgrade.

## Example

`harness-gate explain HGPLUGIN-DIGEST`
