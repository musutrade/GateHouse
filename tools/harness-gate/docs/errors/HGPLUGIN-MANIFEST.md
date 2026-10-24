# HGPLUGIN-MANIFEST

## Why this fails

The signed manifest is not a valid `harness-plugin/v1` document, or it failed to decode after signature verification.

## Diagnose and repair

Prepare a data-only v1 preset manifest with the required fields, review compatibility, and lock its exact digest.

## Example

`harness-gate explain HGPLUGIN-MANIFEST`
