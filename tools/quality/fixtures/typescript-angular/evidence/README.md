# GH-129 validation evidence

This record covers OpenSpec tasks 1.1–1.2 only. No adapter is certified.

- `native.tar.gz`: complete native collection; `manifest.json` records exact
  commands, runtime versions, configuration hashes, source bytes, artifact hashes,
  repeated locked-install inventories and tool exit statuses.
- `failed-build.tar.gz`: real Angular build failure with retained failing source,
  argv, output and exit status. Its manifest remains `failed`.
- `validation.tar.gz`: full logs of the requested repository checks and collector
  failure-boundary tests, with commands/statuses in `validation.json`.
- `index.json`: SHA-256 and byte sizes of all three archives.

Inspect without modifying the repository:

```sh
tar -tzf tools/quality/fixtures/typescript-angular/evidence/native.tar.gz
tar -xOzf tools/quality/fixtures/typescript-angular/evidence/native.tar.gz manifest.json
```

Raw native coverage includes intentionally uncovered production branches; test
success is not a coverage threshold or adapter policy decision. The provider's
negative exit status in the complete run is the collector's recorded shutdown
after successful tests, not a provider test result.

Repository validation uses `CARGO_TARGET_DIR=target` where compilation is needed.
The initial unmodified nextest and docs-consistency commands failed because the
environment points at read-only `/home/gem/cargo-target`; their failure records
are retained. The reruns use the workspace target and preserve all command
arguments. Initial npm registry
queries also encountered the read-only default cache; all retained installs use
an explicitly writable cache. A first collection exposed a missing Node type
dependency; the corrected lockfile includes exact `@types/node` and the final
collection runs against it.

Final local results: 315 Rust tests passed; Rust formatting and Clippy passed;
212 Python tests passed; docs consistency passed. The native collection passed
seven Angular tests and one Rust provider test. The deliberately broken Angular
source failed compilation with exit status 1 and retained a failed manifest.

Submission encountered a read-only workspace `.git/index.lock`. Git metadata was
copied into ignored `target/quality/gh129/delivery.git` to commit and push the same
branch from this workspace. The primary read-only Git metadata retains its
original revision; the handoff identifies the actual pushed commit.

`harness-gate config check` and `harness-gate verify --profile ci --all` are **not
applicable**: this checkout has no `.harness-gate/flow.toml` declaring a `ci`
profile. Hosted required CI remains pending at submission and must pass before
merge; the controller owns CI observation and delivery.
