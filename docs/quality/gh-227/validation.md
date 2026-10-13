# GH-227 contract validation

Scope: P0.1, P0.2, P1.1, P1.2 only. Relevant Engineering Policy semantics remain
unchanged. Assigned workspace: `/mnt/dev-ssd/workspaces/symphony/GH-227`, branch
`symphony/GH-227`, starting `origin/main`/HEAD
`c8203fbee33298440de2e73e217a53fbaa93dd10`. No other checkout was accessed.
GitHub API confirmed that main SHA and PR #226's accepted merge
`94b1243f25275b26b2edf3d11f0e12c28eb16eaa` before implementation.
Planning receipts are linked in the [design](../../../openspec/changes/package-official-rust-collector-for-independent-delivery/design.md#validation-status).
All four task estimates remain at most three focused hours; no larger item was
implemented. Retained build/capture outputs for this task are under `target/gh-227/`.
Initial legacy Python tests inherited `/mnt/dev-ssd/dev-tmp` for disposable test
scratch; no historical evidence there was accessed or used. The final run sets
`TMPDIR` to this workspace explicitly and retains its output here.

## Changes and interpretation

- P0.1: [source inventory and authority boundaries](../rust-collector-delivery-contract.md#existing-boundaries-and-exits).
- P0.2: typed measurement completion/failure, unchanged legacy exits, low-coverage,
  all capability states, incomplete evidence and nonzero subprocess tests.
- P1.1: strict manifest/schema and exact compatibility preflight; production matrix
  empty, rejecting every untested delivery combination.
- P1.2: capture identity guard and relocation negatives; reviewed transition
  requirements preserve original paths/bytes, anchors, series and Core lineage.

No standalone runtime, real native positive capture, ABI certification, release,
baseline acceptance, frozen oracle edit or GH-215 transfer is claimed. Synthetic
fixtures and mocked native reports establish contract behavior only. Native suites
that need `NATIVE_DRIVER`/`NATIVE_DRIVER_SYSROOT` cannot establish acceptance when
those dependencies are absent. Future P2–P8 acceptance remains unchecked.
No new CI producer or sampling cost is introduced; the new contract tests take
less than one second locally and use no native compiler.

## Commands and results

Commands ran from the assigned workspace. Full local outputs are retained in
`target/gh-227/`; complete compressed copies are retained in [validation-logs.tar.gz](https://github.com/musutrade/Harness-Gate/blob/d9a932b23dcfea709175285b594ca200450a9971/docs/quality/gh-227/validation-logs.tar.gz).
Archive members use the log names below; original failures are included.
CI and controller acceptance are pending for this implementation PR.

| Exact command | Result | Log |
| --- | --- | --- |
| `python3 -m unittest discover -s tools/quality/tests -p test_collector_runner.py -v` | Exit 0; 12 tests passed before changes, 2.727s | `baseline-protocol.log` |
| `python3 -m unittest discover -s tools/quality/tests -p test_rust_collector_contract.py -v` | Initial exit 1: 10 tests, one failure from wrong synthetic metric field (`metric` instead of `name`). Test fixture corrected. Subsequent 11 tests passed; final run after protocol/Core schema additions exit 0. | `contract-initial.log`, `contract-final.log`, `contract-updated.log` |
| `CARGO_TARGET_DIR="$PWD/target/gh-227/cargo" cargo nextest run --manifest-path tools/harness-gate/Cargo.toml --locked` | Exit 100; 354 passed, one localhost webhook failed with connection refused, 37 not run due to fail-fast; 177.963s | `nextest.log` |
| `env -u HTTP_PROXY -u http_proxy -u HTTPS_PROXY -u https_proxy -u ALL_PROXY -u all_proxy CARGO_TARGET_DIR="$PWD/target/gh-227/cargo" cargo nextest run --manifest-path tools/harness-gate/Cargo.toml --locked -E 'test(webhook_accepts_success_response)'` | Exit 0; one test passed after command-local proxy removal. No test or global environment changed. | `nextest-webhook-retry.log` |
| `env -u HTTP_PROXY -u http_proxy -u HTTPS_PROXY -u https_proxy -u ALL_PROXY -u all_proxy CARGO_TARGET_DIR="$PWD/target/gh-227/cargo" cargo nextest run --manifest-path tools/harness-gate/Cargo.toml --locked --no-fail-fast` | Exit 0; all 392 tests passed, zero skipped, 174.784s | `nextest-final.log` |
| `CARGO_TARGET_DIR="$PWD/target/gh-227/cargo" cargo fmt --manifest-path tools/harness-gate/Cargo.toml -- --check` | Exit 0 | `fmt.log` (empty output) |
| `CARGO_TARGET_DIR="$PWD/target/gh-227/cargo" cargo clippy --manifest-path tools/harness-gate/Cargo.toml --all-targets -- -D warnings` | Exit 0 | `clippy.log` |
| `python3 -m unittest discover -s tools/quality/tests -v` | Initial failure: 401 tests in 187.889s, one error (documentation link before validation.md existed), one failure (new module missing from Python inventory), two native class skips. Added the evidence document and inventory entries; frozen helpers unchanged. | `python-unittest.log` |
| `CARGO_TARGET_DIR="$PWD/target/gh-227/cargo" python3 tools/quality/docs_consistency.py --output target/quality/docs-consistency.json` | Initial exit 1: missing `gh-227/validation.md` link in the new contract. This document fixes the missing target. Final rerun exit 0, status pass and no broken links. | `docs-consistency.log`, `docs-consistency-initial.json` |
| `TMPDIR="$PWD/target/gh-227/tmp" CARGO_TARGET_DIR="$PWD/target/gh-227/cargo" cargo clippy --manifest-path tools/harness-gate/Cargo.toml --locked --all-targets --all-features -- -D warnings` | Exit 0; PR template variant also passed. | `clippy-template.log` |
| `openspec validate package-official-rust-collector-for-independent-delivery --strict --no-interactive` | Printed change valid, but tool call failed when network policy blocked `https://edge.openspec.dev:443`; no successful exit claimed for this invocation. | `openspec.log`, `tool-errors.txt` |
| `OPENSPEC_TELEMETRY=0 openspec validate package-official-rust-collector-for-independent-delivery --strict --no-interactive` | Exit 0, change valid; installed CLI 1.10.0, telemetry/version network checks disabled for this command only. | `openspec-final.log` |

The schema also passed installed `jsonschema.Draft7Validator.check_schema` and
validation of the explicitly synthetic Manifest, Matrix and Environment fixtures;
the exact inline command is retained as `schema-check.py` and output as `schema.log`.
This checks schema syntax, not native compatibility or signature authentication.

`harness-gate config check` and `harness-gate verify --profile ci --all` are **not
applicable**: this checkout has no `.harness-gate/flow.toml` declaring a `ci`
profile. No generic project configuration was created to bypass that condition.

Shell `git ls-remote origin refs/heads/main` failed with
`Failed to connect to github.com port 443 via 192.168.0.34 after 0 ms: Could not connect to server`.
`env -u HTTP_PROXY -u http_proxy -u HTTPS_PROXY -u https_proxy -u ALL_PROXY -u all_proxy git ls-remote origin refs/heads/main`
also failed through the configured proxy; `git -c http.proxy= -c https.proxy= ls-remote origin refs/heads/main`
failed with `Could not resolve host: github.com` (both exit 128; logs
`git-connectivity.log` and `git-direct-connectivity.log`).
`git add -- tools/quality/rust_collector_contract.py` failed with exit 128:
`fatal: Unable to create '/mnt/dev-ssd/workspaces/symphony/GH-227/.git/index.lock': Read-only file system`
(`git-stage.log`). The configured GitHub API is the available delivery transport:
create a tree/commit with the assigned starting commit as parent, publish only
`refs/heads/symphony/GH-227`, and open a PR targeting main. Local read-only Git
metadata remains at the starting SHA; the handoff identifies the published remote
commit. No original Git metadata, other checkout, or delivery-base ref is modified.

## Final documentation/Python validation

`CARGO_TARGET_DIR="$PWD/target/gh-227/cargo" python3 -m unittest discover -s tools/quality/tests -v`
reported `Ran 401 tests in 175.610s`, `OK (skipped=2)` after both fixes
(`python-unittest-final.log`). The final command was:
```sh
TMPDIR="$PWD/target/gh-227/tmp" CARGO_TARGET_DIR="$PWD/target/gh-227/cargo" python3 -m unittest discover -s tools/quality/tests -v
```
Exit 0: 401 tests in 178.562s, `OK (skipped=2)`
(`python-unittest-workspace.log`). The two native test classes require `NATIVE_DRIVER` and
`NATIVE_DRIVER_SYSROOT`; their skip is not a native positive measurement.

Final documentation validation also used workspace-local `TMPDIR`:
```sh
TMPDIR="$PWD/target/gh-227/tmp" CARGO_TARGET_DIR="$PWD/target/gh-227/cargo" python3 tools/quality/docs_consistency.py --output target/quality/docs-consistency.json
```
Exit 0, status pass, no link failures. The final JSON is retained in the archive
as `docs-consistency-final.json`. `git diff --check` passed.

Required hosted CI,
review, merge and issue completion belong to the controller; this issue remains
open at submission.
