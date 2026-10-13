# GH-115: baseline, debt and no-regression ratchet validation

Scope: OpenSpec `language-agnostic-evidence-policy-architecture` tasks 6.1–6.4.
Dependency GH-114 is closed and PR #123 merged at
`1dc5db536a00917d8bbc3f2704b6ed2bfbf0c2ea`, the checkout's starting commit.

The [policy contract](policy-engine.md) documents the implementation. The
[acceptance tests](../../tools/quality/tests/test_policy_ratchet.py) cover:

- Exact identities remain unchanged; explicit content edits, renames and moves
  are modified. Input ordering does not affect classification. Missing, ambiguous
  or incompatible history blocks comparison; split children receive no debt allowance.
- Across four synthetic ecosystem fixtures, CRAP 64 → 55 remains improved debt
  while a new subject at 31 fails the same maximum-30 policy. Unchanged debt,
  worsening debt, compliant regression, resolved debt and both ratchet options
  have explicit outcomes. Exact ratios and boolean comparisons use the same engine.
- Missing base evidence, unavailable metrics, series drift and project, commit or
  target mismatches block incremental success.
- Owner, issue, reason, expiry and compensating control are required. Invalid,
  expired or duplicate exception metadata blocks review; valid metadata never
  changes a failed quality result to pass. The CLI retains the debt ledger.

The pre-change evaluator suite passed 9 tests. Final local validation is recorded
in the [machine summary](https://github.com/musutrade/Harness-Gate/blob/d9a932b23dcfea709175285b594ca200450a9971/docs/quality/gh-115/validation-summary.json); complete command logs
and a representative improved-debt/new-failure report are retained in the
[evidence archive](https://github.com/musutrade/Harness-Gate/blob/d9a932b23dcfea709175285b594ca200450a9971/docs/quality/gh-115/local-evidence.tar.gz).

| Command | Actual result |
| --- | --- |
| `cargo nextest run --manifest-path tools/harness-gate/Cargo.toml --locked` | Passed: 315 tests, 0 skipped |
| `cargo fmt --manifest-path tools/harness-gate/Cargo.toml -- --check` | Passed |
| `cargo clippy --manifest-path tools/harness-gate/Cargo.toml --all-targets -- -D warnings` | Passed |
| `python3 -m unittest discover -s tools/quality/tests -v` | Passed: 177 tests |
| `python3 tools/quality/docs_consistency.py --output target/quality/docs-consistency.json` | Passed: links, schemas, examples and wording |
| `harness-gate config check` | Not applicable: no `.harness-gate/flow.toml` or declared `ci` profile |
| `harness-gate verify --profile ci --all` | Not applicable: no `.harness-gate/flow.toml` or declared `ci` profile |

Rust validation uses `CARGO_TARGET_DIR` under this workspace's
`target/gh-115-cargo`. The fixtures are synthetic contract evidence, not adapter
certification or whole-project compliance. The engine remains standalone shadow
mode; production baseline acceptance, Rust migration, required-gate equivalence
and remaining proposal tasks are not accepted here.
[ADR-0039](../adr/0039-required-risk-and-traceability-gates.md) remains the required
gate authority. Hosted required CI is pending and must pass before merge; the
controller owns merge and issue closure.

Delivery environment: the initial `git add` failed with exit 128:
`fatal: Unable to create '/home/gem/symphony-workspaces/GH-115/.git/index.lock': Read-only file system`.
A copy of this workspace's Git metadata at `target/gh-115-git` is used with
`--git-dir=target/gh-115-git --work-tree=.` for commit and push on the same branch.
No other checkout is accessed.
