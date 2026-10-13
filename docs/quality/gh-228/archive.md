# GH-228 archived evidence

Use the [archive index and verified restoration guide](../archived-evidence.md)
to recover original bytes. Search the index for the full path under
`docs/quality/gh-228/`; download the containing record's archive and restore the
selected members into a new directory.

- The nineteen `native-evidence.tar.gz.part-*` files belong to the unchanged
  2026-10-10 archive. Individual text placeholders are no longer checked in.
- `validation-logs.tar.gz` and `resumed-validation-logs.tar.gz` belong to the
  2026-10-13 supplemental archive.
- [The original inventory](https://github.com/musutrade/Harness-Gate/blob/d9a932b23dcfea709175285b594ca200450a9971/docs/quality/gh-228/retained-evidence.json),
  [transport validation](https://github.com/musutrade/Harness-Gate/blob/d9a932b23dcfea709175285b594ca200450a9971/docs/quality/gh-228/transport-validation.json),
  [validation results](https://github.com/musutrade/Harness-Gate/blob/d9a932b23dcfea709175285b594ca200450a9971/docs/quality/gh-228/resumed-validation.json) and the dated reports remain
  checked in with their original bytes and outcomes.

Restore all nineteen native parts before following the historical concatenation
instructions in [transport.md](transport.md). Their original order, sizes and
SHA-256 values remain authoritative. Neither archive relocation nor a successful
restore changes any historical failed validation into a pass.
