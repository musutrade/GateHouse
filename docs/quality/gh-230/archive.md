# GH-230 archived evidence

Use the [archive index and verified restoration guide](../archived-evidence.md)
to recover original bytes. Search for the full path under `docs/quality/gh-230/`.
Historical paths in JSON inventories identify archive members after relocation.

The 134 `*.tar.gz.parts/part-*` members belong to the unchanged 2026-10-10
archive. Their individual text placeholders are removed. The original
[part-manifest directory](https://github.com/musutrade/Harness-Gate/blob/d9a932b23dcfea709175285b594ca200450a9971/docs/quality/gh-230/evidence-parts.json), each `manifest.json`, file
inventory, archive digest receipt and failure/result summary remain checked in.
Restore all parts for the desired bundle, concatenate in manifest order and
verify the original assembled archive digest before using it.

The 2026-10-13 supplement contains these original standalone files:

- `configured-core-validation-logs.tar.gz`
- `core-0.4.0-validation-logs.tar.gz`
- `final-validation-logs.tar.gz`
- `invocation-error-evidence.tar.gz`
- `malformed-core-validation-logs.tar.gz`
- `validation-logs.tar.gz`

Their existing receipts and inventories describe the same bytes. The historical
[acceptance](acceptance.md), [genuine failures](validation.md) and
[final validation](final-validation.md) retain their dates and scope.
