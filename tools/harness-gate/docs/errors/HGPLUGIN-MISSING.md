# HGPLUGIN-MISSING

## Why this fails

A prepared input named by the lock or manifest is absent from the selected offline store, or the signed files do not contain the preset catalog.

## Diagnose and repair

Explicitly prepare or install the exact locked package in the store. Preflight is offline and never downloads missing inputs.

## Example

`harness-gate explain HGPLUGIN-MISSING`
