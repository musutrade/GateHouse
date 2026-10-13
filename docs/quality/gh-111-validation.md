# GH-111 validation record

Scope: OpenSpec `language-agnostic-evidence-policy-architecture` tasks 0.3 and
1.1–1.4 only. Current Rust gates and ADR-0039 remain authoritative. The generic
model is synthetic and does not claim real ecosystem collection or equivalence.

Before changes, `python3 -m unittest tools.quality.tests.test_ci_quality
tools.quality.tests.test_quality_evidence -v` passed 32 tests. The fixture-specific
suite passed 9 tests; full quality unittest discovery passed 128 tests.
`cargo fmt --manifest-path tools/harness-gate/Cargo.toml -- --check` passed.

The initial commands `cargo nextest run --manifest-path
tools/harness-gate/Cargo.toml --locked` and `cargo clippy --manifest-path
tools/harness-gate/Cargo.toml --all-targets -- -D warnings` each exited 101:

```text
error: failed to open: /home/gem/cargo-target/debug/.cargo-build-lock
Caused by:
  Read-only file system (os error 30)
```

This environment points Cargo at an unwritable shared target directory. Validation
uses `CARGO_TARGET_DIR=$PWD/target/gh-111-build` for workspace-local build outputs;
no Cargo configuration or project-local gate configuration is created.

## Final local results

| Command | Result |
| --- | --- |
| `CARGO_TARGET_DIR="$PWD/target/gh-111-build" cargo nextest run --manifest-path tools/harness-gate/Cargo.toml --locked` | Exit 0; 315 passed, 0 skipped |
| `cargo fmt --manifest-path tools/harness-gate/Cargo.toml -- --check` | Exit 0 |
| `CARGO_TARGET_DIR="$PWD/target/gh-111-build" cargo clippy --manifest-path tools/harness-gate/Cargo.toml --all-targets -- -D warnings` | Exit 0 |
| `python3 -m unittest discover -s tools/quality/tests -v` | Exit 0; 128 passed, including 9 generic-model tests with negative subcases |
| `CARGO_TARGET_DIR="$PWD/target/gh-111-build" python3 tools/quality/docs_consistency.py --output target/quality/docs-consistency.json` | Exit 0; examples, migration, schemas and links pass |

The initial docs consistency command without the Cargo override exited 1 with
`quality gate failed: documentation, examples, or schema synchronization failed`.
Its retained report marks examples/migration failed and schema synchronization
false. The script suppresses individual Cargo errors; the successful rerun with
only the target directory changed is consistent with the shared-target limitation.

The model tests cover all four components without language-specific validation,
both source snapshots' actual SHA-256 digests, same short names with distinct
qualified identities, unknown/duplicate graph and subject references, missing or
conflicting digests, canonical paths/spans, and rename/move/split lineage.
Ambiguous destinations, invalid mappings and any unmapped changed identity reject
baseline lookup. Mapping success supplies lineage only, never accepted debt or
metric inheritance.

Retained evidence: [command manifest](https://github.com/musutrade/Harness-Gate/blob/d9a932b23dcfea709175285b594ca200450a9971/docs/quality/gh-111/validation.json),
[complete logs](https://github.com/musutrade/Harness-Gate/blob/d9a932b23dcfea709175285b594ca200450a9971/docs/quality/gh-111/validation.tar.gz), and
[docs consistency report](https://github.com/musutrade/Harness-Gate/blob/d9a932b23dcfea709175285b594ca200450a9971/docs/quality/gh-111/docs-consistency.json). The manifest hashes the
logs and changed contract/fixture files. Reports identify the pre-implementation
checkout commit; the PR commit contains the implementation and these records.
The existing Rust implementation, quality workflows, evidence schema and release
consumer are unchanged. This is local validation, not remote required-CI or
architecture/equivalence acceptance. ADR-0039 remains authoritative.

`harness-gate config check` and `harness-gate verify --profile ci --all` are
**not applicable**, not passed: `.harness-gate/flow.toml` is absent, so there is no
declared project-local `ci` profile.

## Delivery environment

The initial `git add` exited 128: `fatal: Unable to create
'/home/gem/symphony-workspaces/GH-111/.git/index.lock': Read-only file system`.
Delivery uses a copy of this checkout's Git metadata at `target/gh-111-git`, with
`--git-dir=target/gh-111-git --work-tree=.`. It preserves the prepared branch and
base and commits these same workspace files. The original read-only `.git` HEAD
stays at the checkout base; the runtime handoff records the actual pushed SHA.
