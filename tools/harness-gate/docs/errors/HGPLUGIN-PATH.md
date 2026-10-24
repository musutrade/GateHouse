# HGPLUGIN-PATH

## Why this fails

A plugin input or signed artifact path is absolute, contains a parent component, uses a forbidden separator, or is a symlink or non-regular file.

## Diagnose and repair

Use confined relative paths inside the prepared package and prepare regular files. Do not rely on links outside the store.

## Example

`harness-gate explain HGPLUGIN-PATH`
