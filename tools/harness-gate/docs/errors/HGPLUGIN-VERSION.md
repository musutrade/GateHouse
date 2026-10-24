# HGPLUGIN-VERSION

## Why this fails

A pin has an unsafe name, non-numeric or non-exact version, empty source, or the signed manifest name/version/source differs from the lock.

## Diagnose and repair

Use a safe name and exact `major.minor.patch` version, then install the exact locked product. Never silently upgrade.

## Example

`harness-gate explain HGPLUGIN-VERSION`
