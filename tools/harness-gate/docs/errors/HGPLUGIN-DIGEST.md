# HGPLUGIN-DIGEST

## Why this fails

The prepared manifest or a signed artifact does not match the digest pinned in the lock.

## Diagnose and repair

Restore the exact pinned file from the reviewed package. Do not change the lock to hide corruption or silently accept an upgrade; review and re-lock a new release explicitly.

## Example

`harness-gate explain HGPLUGIN-DIGEST`
