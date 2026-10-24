# HGPLUGIN-PROTOCOL

## Why this fails

The signed manifest declares a protocol other than the supported data-only `harness-preset-catalog/v1`.

## Diagnose and repair

Select a data-only preset package. Adapter execution still requires its separately signed v2 requests and is not authorized by a preset pin.

## Example

`harness-gate explain HGPLUGIN-PROTOCOL`
