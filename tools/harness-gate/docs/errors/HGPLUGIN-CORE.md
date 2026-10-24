# HGPLUGIN-CORE

## Why this fails

The signed preset manifest's `core_versions` list does not contain this exact Core version. This is a declaration check on the preset manifest only; it does not validate adapter requests or download a different package.

## Diagnose and repair

Select a reviewed plugin release whose signed preset manifest lists this exact Core version, or explicitly upgrade Core and review the compatibility change. Do not edit the lock to bypass the declaration.

## Example

`harness-gate explain HGPLUGIN-CORE`
