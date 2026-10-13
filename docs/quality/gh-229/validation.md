# GH-229 validation and controller handoff

Scope: P4.1, P4.2, P5.1, P5.2 only. Each remained within its <=3 focused-hour
estimate; no larger implementation task was introduced. GH-228 was accepted and
merged through PR #234 before work began. The assigned `symphony/GH-229` checkout
started at `9bdc203c21a75cc769fdb2adbafb897e4b96a30e`, matching `origin/main`.
The existing builder produced an archive but had no secure installer or
independent release workflow; these entries are added by this issue.

The [installation and release contract](../rust-collector-installation.md)
documents assets, host trust bootstrap, transactions and production prerequisites.
No Core policy semantics, frozen oracle, project tests, compatibility matrix,
baseline or global toolchain were changed. P6/P7/P8 remain unchecked.

## Commands and actual results

Run from this assigned workspace. Rust outputs use
`CARGO_TARGET_DIR="$PWD/target/gh-229/cargo"`. Tests use
`TMPDIR="$PWD/target/gh-229/tmp"`; Git-using suites additionally set
`GIT_CEILING_DIRECTORIES="$PWD/target/gh-229/tmp"` so temporary fixture repositories
do not accidentally discover the containing checkout. The final Rust suite
removes `HTTP_PROXY`, `HTTPS_PROXY`, `ALL_PROXY` and their lowercase equivalents
only for its process, allowing the existing localhost webhook test to connect.
No user/global configuration is changed.

| Command | Actual result |
| --- | --- |
| `cargo nextest run --manifest-path tools/harness-gate/Cargo.toml --locked` | Exit 0; 392 passed, 0 skipped |
| `cargo fmt --manifest-path tools/harness-gate/Cargo.toml -- --check` | Exit 0 |
| `cargo clippy --manifest-path tools/harness-gate/Cargo.toml --all-targets -- -D warnings` | Exit 0 |
| `python3 -m unittest discover -s tools/quality/tests -v` | Exit 0; 402 tests, 3 skipped classes |
| `python3 -m unittest discover -s tools/release/tests -v` | Exit 0; 42 tests |
| `bash tools/release/tests/test_install.sh` | Exit 0; installer integrity tests pass |
| `python3 tools/release/collector_dry_run.py --output target/gh-229/dry-run-final` | Exit 0; synthetic signed install/select/uninstall rehearsal |
| `python3 tools/quality/docs_consistency.py --output target/quality/docs-consistency.json` | Exit 0; workspace-local environment |
| `openspec validate package-official-rust-collector-for-independent-delivery --strict --no-interactive` | Exit 0; CLI 1.10.0, `OPENSPEC_TELEMETRY=0` |
| `git diff --check` | Exit 0 |
| `harness-gate config check` | NOT APPLICABLE: no `.harness-gate/flow.toml` |
| `harness-gate verify --profile ci --all` | NOT APPLICABLE: no `.harness-gate/flow.toml` or declared `ci` profile |

The skipped native classes report their exact prerequisites:
`StandaloneNativeTests`: `RUST_COLLECTOR_RUNTIME required: assembled private Linux runtime`;
`NativeFileClassificationTests`: `NATIVE_DRIVER and NATIVE_DRIVER_SYSROOT required for real file classification`;
`NativeDriverTests`: `NATIVE_DRIVER and NATIVE_DRIVER_SYSROOT required: pinned rustc-dev compiler fixture`.
Those inputs are absent in this checkout. No external historical workspace was
accessed to supply them. These skips are not native positive results. P4/P5's
synthetic installer tests do not rerun P2/P3 native acceptance or certify P7.

## Genuine failures and corrections

1. Initial delivery tests failed two subtests: appending bytes to the RSA signature
   was accepted by OpenSSL. The installer now enforces the independently pinned
   signature width before verification. The initial command used a shell tail
   wrapper that returned 0 even though unittest reported failure; the raw log
   retains both failing assertions. The corrected 12-test delivery rerun passed,
   followed by the complete 41-test and final 42-test release suites.
2. First nextest run exited 100: `test_scope_without_git` unexpectedly succeeded
   because workspace-local temporary directories discovered this checkout's Git
   repository. It ran 40/392 tests: 39 passed, 1 failed, 352 not run. Setting
   `GIT_CEILING_DIRECTORIES` fixes fixture isolation without changing the test.
3. Second nextest run exited 100: `webhook_accepts_success_response` reported
   `io: Connection refused` while proxy variables were inherited. It ran 355/392:
   354 passed, 1 failed, 37 not run. The predecessor documented the same localhost
   proxy constraint. The final run removes those six variables; no check is skipped.
4. The first docs command omitted the workspace-local build/temp overrides and
   exited 1 with failed example/migration/schema checks. Its report and log are
   retained. The tool suppresses child-command stderr, so no more specific cause
   is claimed. Rerunning with the required local environment passed.
5. The first strict OpenSpec command printed that the change was valid, but the
   execution tool rejected telemetry network access to `https://edge.openspec.dev:443`.
   This attempt is not recorded as passed. Disabling telemetry with
   `OPENSPEC_TELEMETRY=0` produced a confirmed exit 0.

## Retained evidence and limits

`validation.json` records exact command arguments, environment overrides, results
and log hashes; `validation-logs.tar.gz` retains complete logs including failures.
`dry-run-evidence.tar.gz` retains both original and final local rehearsal outputs,
including their exact six signed assets, disposable public keys, test trust and
reports. No private signing key is retained. `retained-evidence.json` binds the
archives and every member's digest. These archives are committed, rather than
depending on ignored workspace files or another checkout.

The final local rehearsal's inventory SHA-256 is
`7cdb985f2a98062008cc66455e36ca5ec8f12588344c80ae445e62c72187d106`.
Its report explicitly says production eligibility was not evaluated, protected
approval was not requested, native measurement was not performed, no tag was
created and publication was not attempted. Eligibility tests use mocked GitHub
responses; inherited Core release tests exercise actual temporary Git tag/main
relationships. No repository release tag or RC was created by this issue.

Lifecycle evidence covers every injected activation boundary, actual SIGKILL,
interrupted uninstall, idempotent recovery, two-version rollback, corrupt rollback,
unrelated evidence preservation, all six missing/tampered assets, unsigned assets,
wrong key/tag/ABI, signed inconsistent provenance/SBOM, payload digest/mode errors,
traversal, links, archive types, duplicates, missing/extra and trailing payloads.
Environment tests reject missing reviewers, self-review and administrator bypass;
workflow tests require protected read-only execution and no publication path.

The controller owns required hosted CI, review, merge and issue completion.
Submission is **CI pending**, not delivery or production release approval. GH-230
remains the already-authorized serial successor after acceptance and merge.
