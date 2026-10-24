# HGPLUGIN-STORE

## Why this fails

No `--plugin-store` was supplied, or the selected store path cannot be resolved.

## Diagnose and repair

Pass `--plugin-store PATH` pointing to explicitly prepared offline packages. Preflight does not create or populate the store.

## Example

`harness-gate explain HGPLUGIN-STORE`
