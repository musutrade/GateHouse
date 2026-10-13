# TypeScript identity and native counter fixture

GH-130 implements OpenSpec tasks 2.1–2.2 using the frozen GH-129 native
collection. No collector certification or generic policy integration is claimed.
The [semantic contract](../../../../../docs/quality/typescript-source-semantics.md)
records TS-01/TS-02, capability states and series compatibility.

- `source-index.json`: original-source AST inventory produced by the pinned
  TypeScript 6.0.2 compiler API, including lexical owners and UTF-16 spans.
- `native-counters.json`: exact native covered/total counters from
  `istanbul-lib-coverage` for each file and AST-local function/method scope.
  Percentages are deliberately omitted. Scope restriction is explicit in
  [coverage-oracle.cjs](../coverage-oracle.cjs).
- `receipt.json`: caller-owned trust input pinning the native archive and AST
  index. Its collection-root string translates native absolute paths; replay
  never opens that historical directory. Updating a receipt requires reviewing
  the collected artifacts, not merely accepting hashes from an incoming report.

Reproduce from the repository root with the versions in
[toolchain.json](../toolchain.json):

```bash
cd tools/quality/fixtures/typescript-angular/app
npm ci --no-audit --no-fund --cache /tmp/gh130-npm-cache
cd ../../../../..
mkdir -p target/quality/gh130/native
tar -xzf tools/quality/fixtures/typescript-angular/evidence/native.tar.gz -C target/quality/gh130/native
node tools/quality/fixtures/typescript-angular/source-index.cjs target/quality/gh130/native/sources/app > target/quality/gh130/source-index.json
cmp tools/quality/fixtures/typescript-angular/semantics/source-index.json target/quality/gh130/source-index.json
node tools/quality/fixtures/typescript-angular/coverage-oracle.cjs target/quality/gh130/source-index.json target/quality/gh130/native/coverage/reference-app/coverage-final.json "$(python3 -c 'import json; print(json.load(open("tools/quality/fixtures/typescript-angular/semantics/receipt.json"))["coverage_root"])')" > target/quality/gh130/native-counters.json
cmp tools/quality/fixtures/typescript-angular/semantics/native-counters.json target/quality/gh130/native-counters.json
python3 -m unittest discover -s tools/quality/tests -p test_typescript_semantics.py -v
```

Replay tests require Python only; parser/oracle regeneration requires the pinned
Node dependencies. Five accepted original TypeScript files reproduce all retained
native file/function/method counters. Three same-named methods and two anonymous
arrows have distinct original-source subjects. The native missing-map case,
templates, generated sources and adversarial provenance are tested separately.

See [GH-130 validation evidence](../../../../../docs/quality/gh-130/README.md).
