# HGPLUGIN-PLATFORM

## Why this fails

The signed manifest does not list this exact operating system and architecture.

## Diagnose and repair

Prepare a reviewed package that supports this exact platform; do not force-install a package built for another platform.

## Example

`harness-gate explain HGPLUGIN-PLATFORM`
