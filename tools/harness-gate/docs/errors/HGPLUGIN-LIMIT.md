# HGPLUGIN-LIMIT

## Why this fails

The signed file inventory is outside 1..128 entries, or a single input exceeds the 1 MiB read limit. The inventory size is validated before copying starts; the per-file size limit is enforced as each file is read and copied into the private preflight snapshot.

## Diagnose and repair

Prepare a bounded data-only package: keep the inventory within 1..128 entries and split any input larger than 1 MiB. If a later file fails, earlier files exist only in the private temporary snapshot and are discarded when preflight ends; there is no all-files precheck or cross-file atomicity.

## Example

`harness-gate explain HGPLUGIN-LIMIT`
