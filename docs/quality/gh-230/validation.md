# GH-230 blocked-attempt validation

> Storage update (2026-10-13): historical raw logs/bundles are available through
> the [verified archive guide](archive.md). Dates, digests and outcomes below are unchanged.

This is baseline diagnostic evidence, not completion of P6/P7. See the
[native prerequisite blocker](blocker.md). Production code, task checkboxes and
the empty compatibility matrix are unchanged.

Commands ran from the assigned checkout with `CARGO_TARGET_DIR=$PWD/target/gh-230/cargo`,
`TMPDIR=$PWD/target/gh-230/tmp` and
`GIT_CEILING_DIRECTORIES=$PWD/target/gh-230/tmp`, except the initial documentation
command noted below. Exact command arrays and process
exit codes are in [checks.json](https://github.com/musutrade/Harness-Gate/blob/d9a932b23dcfea709175285b594ca200450a9971/docs/quality/gh-230/checks.json).

- `cargo nextest run --manifest-path tools/harness-gate/Cargo.toml --locked`:
  exit 100; 354 passed, one loopback webhook failed with `io: Connection refused`,
  and 37 tests were not run after failure. No complete-suite pass is claimed.
- The [targeted loopback rerun](https://github.com/musutrade/Harness-Gate/blob/d9a932b23dcfea709175285b594ca200450a9971/docs/quality/gh-230/loopback.json), with proxy environment variables
  removed for the local listener, exited 0: one passed, 391 filtered out.
- `cargo fmt --manifest-path tools/harness-gate/Cargo.toml -- --check`: exit 0.
- `cargo clippy --manifest-path tools/harness-gate/Cargo.toml --all-targets -- -D warnings`:
  exit 0.
- `python3 -m unittest discover -s tools/quality/tests -v`: exit 1,
  402 tests, one documentation error, three skipped native classes. The error
  was in `test_all_presets_migration_and_schema_use_locked_cargo`, during
  assembly of the blocker documentation while its local links were incomplete.
  The skipped classes require `RUST_COLLECTOR_RUNTIME`, `NATIVE_DRIVER` and
  `NATIVE_DRIVER_SYSROOT`; these skips are not native positives.
- `python3 -m unittest discover -s tools/quality/tests -p test_docs_consistency.py -v`:
  exit 0, all five tests passed after completing the referenced documents.
  The full Python suite was not rerun; its original failure remains retained.
- `python3 tools/quality/docs_consistency.py --output target/quality/docs-consistency.json`:
  initial exit 1 without the explicit workspace environment above, with CLI
  example/migration/schema failures and no link failures. The original report
  is [retained](https://github.com/musutrade/Harness-Gate/blob/d9a932b23dcfea709175285b594ca200450a9971/docs/quality/gh-230/docs-consistency-initial.json). Rerun with the explicit environment:
  exit 0, [all checks passed](https://github.com/musutrade/Harness-Gate/blob/d9a932b23dcfea709175285b594ca200450a9971/docs/quality/gh-230/docs-consistency-final.json).
- `OPENSPEC_TELEMETRY=0 openspec validate package-official-rust-collector-for-independent-delivery --strict --no-interactive`:
  exit 0, CLI 1.10.0, change valid.
- `harness-gate config check` and `harness-gate verify --profile ci --all`:
  not applicable; `.harness-gate/flow.toml` is absent. No generic project config
  was invented.

`git diff --check` exited 0 (tracked files remain unchanged). Complete command
logs are retained under `target/gh-230/logs/` and in
[validation-logs.tar.gz](archive.md). Native positive acceptance has
not run, and required CI has not run for this unfinished attempt. No complete
local validation pass or acceptance is claimed. The environment input described
in the blocker must be supplied before implementation resumes.
