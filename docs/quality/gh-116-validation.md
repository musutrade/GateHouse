# GH-116: Rust reference adapter validation

Scope: OpenSpec `language-agnostic-evidence-policy-architecture` tasks 7.1–7.5.
Dependency GH-115 is closed and PR #124 merged at
`f00d001d4ad8933a37c6b748ae44cde369656597`, this checkout's starting commit.

The [adapter contract](rust-reference-adapter.md) describes the projection and
authority boundary. Historical tests verify production aggregate raw lines
9844/11470, functions 830/1114 and regions 14039/17037; all file source digests;
212 function decisions, including 75 legacy-debt classifications; base/head
native row identity, exact/display CRAP and CC; and unsupported branches without
numeric values. A subprocess prohibition proves that replay does not recollect.

Failed-gate derivations preserve raw counts. Changed-function fixtures preserve
the native low/high-CC ratchet and debt semantics. Missing/tampered evidence,
stale identities, unknown contracts, malformed values, duplicate keys and
injected policy disagreement block migration. Failed/cancelled/skipped/errored
required stage results cannot turn green when metric projections pass.

The accepted Phase 1 baseline predates this candidate format. Its historical
fixture preserves bytes and series and rejects reinterpretation; the GH-97
archive remains a passing candidate, not an accepted baseline. No new expensive
coverage collection or production baseline acceptance is claimed.

Before changes, the Python quality suite passed 177 tests. Actual final commands
and outcomes are recorded in the [machine summary](https://github.com/musutrade/Harness-Gate/blob/d9a932b23dcfea709175285b594ca200450a9971/docs/quality/gh-116/validation-summary.json).
Complete validation logs and a compact historical shadow report are retained in
the [evidence archive](https://github.com/musutrade/Harness-Gate/blob/d9a932b23dcfea709175285b594ca200450a9971/docs/quality/gh-116/local-evidence.tar.gz).

| Command | Actual result |
| --- | --- |
| `cargo nextest run --manifest-path tools/harness-gate/Cargo.toml --locked` | Passed: 315 tests, 0 skipped |
| `cargo fmt --manifest-path tools/harness-gate/Cargo.toml -- --check` | Passed |
| `cargo clippy --manifest-path tools/harness-gate/Cargo.toml --all-targets -- -D warnings` | Passed |
| `python3 -m unittest discover -s tools/quality/tests -v` | Passed: 190 tests |
| `python3 tools/quality/docs_consistency.py --output target/quality/docs-consistency.json` | Passed |
| `openspec validate language-agnostic-evidence-policy-architecture --strict` | Passed |
| `harness-gate config check` | Not applicable: `.harness-gate/flow.toml` is absent |
| `harness-gate verify --profile ci --all` | Not applicable: no project-local `ci` profile |

Cargo build commands use `CARGO_TARGET_DIR=$PWD/target/gh-116-cargo`, inside this
workspace. No project configuration was invented. Hosted required CI remains
pending and must pass before merge. [ADR-0039](../adr/0039-required-risk-and-traceability-gates.md)
remains authoritative; no required gate, ruleset or workflow is replaced.

Delivery environment: `git add tools/quality/rust_reference.py` returned exit 128:
`fatal: Unable to create '/home/gem/symphony-workspaces/GH-116/.git/index.lock': Read-only file system`.
A copy of this workspace's Git metadata at `target/gh-116-git` is used with
`--git-dir=target/gh-116-git --work-tree=.` for commit/push on the same branch.
No other checkout is accessed.
