# GH-230 final local validation

> Storage update (2026-10-13): historical raw logs/bundles are available through
> the [verified archive guide](archive.md). Dates, digests and outcomes below are unchanged.

See [acceptance](acceptance.md) for native evidence and scope. Commands below ran
against integrated main `ee544690662645806cda7dd4cd2c9192566f7929` plus this PR's
source edits. Hosted required CI and controller acceptance remain pending.

| Exact command | Final result |
| --- | --- |
| `cargo nextest run --manifest-path tools/harness-gate/Cargo.toml --locked` | PASS, 392 passed, 0 skipped; 204.908 s test summary |
| `cargo fmt --manifest-path tools/harness-gate/Cargo.toml -- --check` | PASS, exit 0 |
| `cargo clippy --manifest-path tools/harness-gate/Cargo.toml --all-targets -- -D warnings` | PASS, exit 0 |
| `python3 -m unittest discover -s tools/quality/tests -v` | PASS, 439 tests, 0 skips; corrected command wall 330.149 s |
| `python3 -m unittest discover -s tools/release/tests -v` | PASS, 42 tests |
| `python3 tools/quality/docs_consistency.py --output target/quality/docs-consistency.json` | PASS, exit 0, all reported sections pass |
| `OPENSPEC_TELEMETRY=0 openspec validate package-official-rust-collector-for-independent-delivery --strict --no-interactive` | PASS |
| `git --git-dir=target/gh-230/continued/git-metadata --work-tree=. diff --check` | PASS |
| `harness-gate config check` | Not applicable: no project-local `.harness-gate/flow.toml` with a declared ci profile |
| `harness-gate verify --profile ci --all` | Not applicable for the same reason; no invented configuration |

Build/test environment (absolute paths resolve below this assigned workspace):

```text
CARGO_TARGET_DIR=$PWD/target/gh-230/cargo
TMPDIR=$PWD/target/gh-230/final-checks/tmp
GIT_CEILING_DIRECTORIES=$PWD/target/gh-230/final-checks/tmp
RUST_COLLECTOR_RUNTIME=$PWD/target/gh-230/final-acceptance-v3/runtime
RUST_COLLECTOR_TEST_VENDOR=$PWD/target/gh-230/continued/vendor
HARNESS_GATE_NATIVE_POLICY_BINARY=$PWD/target/symphony-inputs/core-v0.4.0/harness-gate-linux-amd64
NATIVE_DRIVER=$PWD/target/gh-230/operator-native-runtime/build/debug/harness-gate-rust-native-driver
NATIVE_DRIVER_SYSROOT=$(rustc --print sysroot)
OPENSPEC_TELEMETRY=0
```

Proxy variables were removed for local servers. The existing read-only `.git`
necessitated workspace-local continuation metadata for integration/commit/push;
only documentation metadata commands received its `GIT_DIR`/`GIT_WORK_TREE`.
Fixture test commands did not inherit these overrides. All build, test temporary
and capture paths are workspace-local. No source checkout or another workspace
was accessed or changed.

The [validation archive](archive.md) contains exact environment,
command/exit records and full original/corrected logs, with an
[archive receipt](https://github.com/musutrade/Harness-Gate/blob/d9a932b23dcfea709175285b594ca200450a9971/docs/quality/gh-230/final-validation-archive.json). Genuine failures are retained:

- Initial nextest: 38 passed, 1 failed, 353 not run. A temporary directory nested
  in this checkout discovered its enclosing Git repository in `scope_without_git`.
  Adding the workspace-local Git ceiling fixed isolation; full rerun passed.
- Initial quality suite: 439 tests, one failure in the Python boundary corpus;
  the new delivery module needed an inventory row. The B-class measurement-only
  inventory was updated, focused corpus checks passed, then all 439 passed.
  Frozen C/D semantic hashes were unchanged.
- An intermediate quality rerun was interrupted after spotting Git metadata
  overrides inappropriate for fixture repositories. The corrected script removes
  those overrides and completed successfully; interruption is not a pass.
- Initial documentation command reported examples/migration/schema failures;
  its subprocess diagnostics were not emitted. The retained report is a failure,
  not inferred proof of a specific root cause. The corrected workspace build/temp
  and proxy environment command passed, then was rerun after final documentation.

- The final documentation pass initially found two links to the validation archive
  before it had been written. Creating the real archive and receipt resolved them;
  the failed link report remains retained.

Earlier failure records under this directory are preserved, including superseded
release and container prerequisite failures. No skipped native test, synthetic
negative, archive-only replay or checkout-built Core is used to establish the
final clean-container positive result. Required CI is delegated to the controller
through the normal PR handoff; P8 completion is not claimed here.
