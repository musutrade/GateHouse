# GH-112 validation record

Scope: OpenSpec `language-agnostic-evidence-policy-architecture` tasks 2.1–2.5 and
3.1–3.4 only. The [contract](harness-evidence.md) describes the new standalone
schema, artifact validation, canonical serialization, typed values, capability
requirements and series identity. Current Rust schema, workflow and ADR-0039
remain unchanged. Real ecosystem adapters and Rust equivalence are not claimed.

Before implementation, the generic validator was absent and the existing project
model suite passed 9 tests. The focused evidence suite now passes 18 tests,
including 16 declarative negative fixture mutations and additional negative
subcases for typed values, duplicate identities, source bytes, symlinks, all six
capability states, availability requirements and all semantic series fields.

## Local validation

The initial exact `cargo nextest run --manifest-path tools/harness-gate/Cargo.toml
--locked` and `cargo clippy --manifest-path tools/harness-gate/Cargo.toml
--all-targets -- -D warnings` commands each exited 101 with:

```text
error: failed to open: /home/gem/cargo-target/debug/.cargo-build-lock
Caused by:
  Read-only file system (os error 30)
```

Cargo's configured shared target is unwritable. Reruns use only a workspace-local
`CARGO_TARGET_DIR`; no project configuration is invented.

| Command | Actual result |
| --- | --- |
| `CARGO_TARGET_DIR="$PWD/target/gh-112-build" cargo nextest run --manifest-path tools/harness-gate/Cargo.toml --locked` | Exit 0; 315 passed, 0 skipped |
| `cargo fmt --manifest-path tools/harness-gate/Cargo.toml -- --check` | Exit 0 |
| `CARGO_TARGET_DIR="$PWD/target/gh-112-build" cargo clippy --manifest-path tools/harness-gate/Cargo.toml --all-targets -- -D warnings` | Exit 0 |
| `python3 -m unittest discover -s tools/quality/tests -v` | Exit 0; 146 passed |
| `python3 tools/quality/docs_consistency.py --output target/quality/docs-consistency.json` | Initial exit 1; Cargo-backed examples/migration and schema sync failed |
| `CARGO_TARGET_DIR="$PWD/target/gh-112-build" python3 tools/quality/docs_consistency.py --output target/quality/docs-consistency.json` | Exit 0; links, examples, migration and schemas pass |
| Normalized evidence CLI command in the contract record | Exit 0; four records valid; canonical SHA-256 retained |
| `harness-gate config check` | Not applicable: no `.harness-gate/flow.toml` |
| `harness-gate verify --profile ci --all` | Not applicable: no `.harness-gate/flow.toml` with declared `ci` profile |

The CLI's `valid` result asserts envelope integrity, not successful collection or
a release pass. The synthetic records intentionally contain measurement errors
for one capability to prove state preservation.

## Retained evidence and scope acceptance

[Machine-readable validation](https://github.com/musutrade/Harness-Gate/blob/d9a932b23dcfea709175285b594ca200450a9971/docs/quality/gh-112/validation.json) retains exact commands,
exit codes, source input digests and not-applicable decisions.
[Contract outcomes](https://github.com/musutrade/Harness-Gate/blob/d9a932b23dcfea709175285b594ca200450a9971/docs/quality/gh-112/contract-cases.json) retain the 16 expected negative
fixture rejections and 12 cross-ecosystem capability requirement results.
[Full local logs and reports](https://github.com/musutrade/Harness-Gate/blob/d9a932b23dcfea709175285b594ca200450a9971/docs/quality/gh-112/local-evidence.tar.gz) include the initial
failures and successful reruns. Reports name the checkout base commit because
validation ran before committing; input digests identify the tested modified files.
The old Rust schema, validator and CI workflow have no diff. Task checkmarks refer to these local contract tests;
required CI is pending. The complete OpenSpec architecture remains unaccepted.


The initial docs check exited 1 with
`quality gate failed: documentation, examples, or schema synchronization failed`.
Its report shows failed Cargo-backed examples/migration and false schema sync;
links and language documentation passed. Changing only `CARGO_TARGET_DIR` made
it pass, consistent with the shared-target build limitation above.

The initial `git add tools/quality/harness_evidence.py` exited 128:
`fatal: Unable to create '/home/gem/symphony-workspaces/GH-112/.git/index.lock': Read-only file system`.
Delivery uses this checkout's Git metadata copied into `target/gh-112-git`, with
`--git-dir=target/gh-112-git --work-tree=.`. It preserves the prepared branch/base
and commits these workspace files. The original read-only `.git` stays at the
checkout base; the runtime handoff records the actual pushed branch and full SHA.
No other workspace or source checkout is used.
