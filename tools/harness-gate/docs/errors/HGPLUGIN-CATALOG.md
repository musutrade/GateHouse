# HGPLUGIN-CATALOG

## Why this fails

The signed preset catalog is missing a referenced flow, recipe or pack file, or its flow configuration is invalid. The catalog is validated only after every signed file has been verified and copied into the private preflight view.

## Diagnose and repair

Restore the exact signed inventory and repair the catalog flow configuration. Re-sign the manifest and files if the catalog itself must change, then explicitly update the lock.

## Example

`harness-gate explain HGPLUGIN-CATALOG`
