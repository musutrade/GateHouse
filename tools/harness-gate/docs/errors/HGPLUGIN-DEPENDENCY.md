# HGPLUGIN-DEPENDENCY

## Why this fails

A plugin dependency is not locked at the required exact version, a program dependency is invalid or duplicated, or a required program is absent from PATH.

## Diagnose and repair

Lock and prepare the exact dependency versions, use plain executable names, and install any reviewed toolchain yourself. Offline preflight never installs or runs a dependency.

## Example

`harness-gate explain HGPLUGIN-DEPENDENCY`
