# HGPLUGIN-LIMIT

## Why this fails

A plugin input exceeds the 1 MiB read limit, or the signed file inventory is outside the supported 1..128 entries.

## Diagnose and repair

Prepare a bounded data-only package and split oversized inputs. Inventory bounds and byte limits are enforced before any file is copied.

## Example

`harness-gate explain HGPLUGIN-LIMIT`
