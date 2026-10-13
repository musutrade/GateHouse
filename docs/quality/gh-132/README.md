# GH-132 validation evidence

Scope: [OpenSpec task 3.2](../../../openspec/changes/typescript-angular-reference-adapter/tasks.md),
following [ADR-0040](../../adr/0040-language-agnostic-evidence-policy.md).
The existing generic policy, lineage, ratchet and debt engines evaluate accepted
TypeScript collector output without any production engine or schema change.

The [integration suite](../../../tools/quality/tests/test_typescript_policy.py)
collects two independently receipt-bound replays of the retained real frontend
archive through the generic collector runner. Native pricing file line coverage
is exactly 4/6: a 4/6 absolute threshold passes, a 5/6 threshold fails, and unchanged
4/6 coverage against the default 4/5 threshold retains visible legacy debt when
policy permits it. The original 11 adapter tests passed before this work.

Controlled counter and identity derivations at the validated policy boundary
exercise regression (including regression above the absolute threshold), improved,
new and resolved debt, denied legacy debt, and modify/rename/move inheritance.
Without explicit lineage, changed identities acquire new debt rather than inheriting
legacy permission. Missing base, ambiguous base/head metric evidence, duplicate
lineage, incompatible tool/normalization/source identity/runtime/rule series and
unavailable base coverage prevent favorable incremental results. Required unsupported
complexity, CRAP, template-associated file and route coverage block the aggregate;
they never acquire numeric values from available file coverage. Violations preserve
component, full subject, metric, typed base/head values, comparison context, series,
artifact links and remediation class.

The same suite projects the retained Rust production coverage with its reference
adapter and aggregates its generic gates alongside the frontend gates. Each
collection keeps its own validated commit/target context. The Rust report remains
exactly equal before and after frontend evaluation, while a frontend threshold
failure blocks the combined aggregate. Rust series cannot supply TypeScript history.
This is policy/gate reuse across two retained ecosystems, not real cross-component
contract integration (task 3.3).

These are retained replays and explicitly derived policy tests, **not new native
base/head executions**. The source archive, source semantics and collector remain
unchanged. Two fresh native acceptance pairs, advisory CI and adapter certification
remain tasks 4.1–4.3. No required Rust gate, release authority or baseline changes.

## Validation

Validation uses `CARGO_TARGET_DIR=$PWD/target` within this workspace. Full command
logs and exit statuses are retained in [validation.tar.gz](https://github.com/musutrade/Harness-Gate/blob/d9a932b23dcfea709175285b594ca200450a9971/docs/quality/gh-132/validation.tar.gz);
[index.json](https://github.com/musutrade/Harness-Gate/blob/d9a932b23dcfea709175285b594ca200450a9971/docs/quality/gh-132/index.json) pins the
archive, integration test and native inputs. Required hosted CI is pending and
must pass before merge.

| Command | Actual result |
| --- | --- |
| `cargo nextest run --manifest-path tools/harness-gate/Cargo.toml --locked` | Exit 0; 315 passed, 0 skipped |
| `cargo fmt --manifest-path tools/harness-gate/Cargo.toml -- --check` | Exit 0 |
| `cargo clippy --manifest-path tools/harness-gate/Cargo.toml --all-targets -- -D warnings` | Exit 0 |
| `python3 -m unittest discover -s tools/quality/tests -v` | Exit 0; 250 tests passed, including all 9 new policy integration tests |
| `python3 tools/quality/docs_consistency.py --output target/quality/docs-consistency.json` | Exit 0; status `pass` |
| `python3 -m unittest discover -s tools/quality/tests -p test_typescript_reference.py -v` | Exit 0; 11 baseline adapter tests passed |

`harness-gate config check` and `harness-gate verify --profile ci --all` are **not
applicable**: this checkout has no `.harness-gate/flow.toml` with a declared `ci`
profile. No project-local configuration was invented.

`git add tools/quality/tests/test_typescript_policy.py` exited 128 with
`fatal: Unable to create '/home/gem/symphony-workspaces/GH-132/.git/index.lock': Read-only file system`.
This workspace's Git metadata was copied into ignored
`target/quality/gh132/delivery.git` for committing and pushing the same branch and
working tree. The original read-only metadata remains at its initial revision;
the handoff declares the actual pushed SHA. The exact failure and workaround
are retained in `delivery-preparation.log`.
