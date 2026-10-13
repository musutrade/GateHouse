# Arc-Admin controlled negatives (GH-205)

OpenSpec tasks 6.1–6.5 are exercised by **disposable boundary fixtures**. Six CLI command receipts and seventeen quality workflow receipts retain the expected failures and their diagnostics. No Arc-Admin checkout, application code, browser assertion, API assertion or smoke-test implementation was changed or copied into Harness-Gate. Project-owned test logic remains in Arc-Admin.

This record follows the [GH-204 shadow observation](../shadow/README.md). Its shared-service configuration blocker (`HG-CAP-001`) and missing trusted collector provisioning remain unresolved here. The unchanged Arc-Admin execution import still declares no `ci` profile. These negative fixtures establish generic orchestration/evidence/policy behavior, not Arc-Admin runtime parity, native application measurements or authority transfer. The whole OpenSpec proposal is not accepted by completing phase D.

## Retained outcomes

| Task | Controlled input | Expected and observed result |
| --- | --- | --- |
| 6.1 | Each imported `frontend.e2e`, `backend.tests`, `frontend.fullstack-smoke` step ID/component binds a disposable `sh` command, required through `policy.required_steps` | Command exit 0 → verify exit 0; command exit 7 → verify exit 1, step/workflow false, command log retained and sealed artifact digests verified |
| 6.2 | Required signed fixture collector exits 17; missing evidence; altered raw artifact; stale evidence context | Quality `blocked`, workflow false. Diagnostics respectively `ADAPTER_PROTOCOL_FAILURE`, missing subject/capability/series, digest mismatch, stale commit/base/target/run |
| 6.3 | Retained baseline request substitutes a stale commit or incompatible tool version | Quality `blocked`, workflow false, no policy report/no fresh baseline substitution. Exact errors: `baseline source/target identity mismatch`; `incompatible baseline profile/tool/measurement series` |
| 6.3–6.4 | Compatible retained baseline CRAP 35, head 40, limit 30, deny-regression and legacy-debt policy enabled | Quality `fail`, workflow false; unchanged series, regression true, debt `regressed`, legacy debt not allowed. Baseline requests remain byte-identical across verification |
| 6.4 | Accepted signed Rust-shaped evidence changes CRAP 10 → 40 against limit 30, with coverage held passing | Paired control `pass`/true; regression `fail`/false and CRAP gate fails. Angular-shaped evidence explicitly declares CRAP `unsupported`, contains no numeric CRAP metric and passes only its supported required policies |
| 6.5 | Explicit partial fixture profiles `hook`, `full`, `ci` omit collectors | Workflow execution true, quality and full-quality `not_collected`, no producer/evidence/raw artifact. An execution success is not full quality PASS |
| 6.5 | Same three profiles require a collector that returns `not_collected` | Quality `fail`, workflow false |
| 6.5 | CLI fixture requests undeclared `ci` after a successful/failed full run | Exit 1, `E1401: unknown or empty verification profile`; previous report unchanged and never attributed to the rejected invocation |

Quality receipts call the same `verify::run` orchestration used by the CLI and assert its returned status; they are not claims of measured quality CLI exit codes. The signed collectors use test keys and synthetic transport fixtures, including the existing Rust/Angular acceptance shapes. The CRAP numbers are controlled policy inputs, not Arc-Admin or native collector measurements. Existing certified Rust and baseline acceptance tests remain part of the required Rust suite; this change does not alter their policies or bypass certification.

## Evidence and replay

- [Command receipts](evidence/commands.json): injected and observed exit codes, original imported step declarations, isolated flow/script, logs, reports, sealed manifests and undeclared-profile diagnostics.
- [Quality receipts](evidence/quality.json): machine reports, human diagnostics, expected workflow/quality outcomes and unchanged-source/baseline assertions.
- [Corpus manifest](manifest.json): SHA-256 hashes of retained receipts. Fixture/workspace paths are normalized for retention. Original sealed artifact digests are checked **before** normalization by the command runner; the manifest hashes the normalized retained files.
- [Validation record](validation.json): exact commands, results and environment limitations.

Check retained evidence without collectors or an Arc-Admin checkout:

```bash
python3 docs/dogfood/arc-admin/negative/reproduce.py
```

Regenerate in this workspace (Linux, Python 3.11+, Git, Rust and cargo-nextest):

```bash
export CARGO_TARGET_DIR="$PWD/target"
cargo build --manifest-path tools/harness-gate/Cargo.toml --locked
python3 docs/dogfood/arc-admin/negative/run.py \
  --harness-gate target/debug/harness-gate --output target/quality/gh-205/replayed
HARNESS_GATE_DOGFOOD_NEGATIVES="$PWD/target/quality/gh-205/replayed" \
  cargo nextest run --manifest-path tools/harness-gate/Cargo.toml --locked \
  -E 'test(arc_admin_controlled_quality)'
```

The CLI runner creates fixtures under `target/quality/gh-205`, checks frozen `sources/`, `import/` and `quality/` hashes before/after, and removes its temporary directories. Rust fixtures use `TempDir`; their source and baseline request bytes are checked before/after execution. No broken application code survives the run. Fresh reports contain new timestamps/run IDs, so compare expected semantics rather than requiring byte equality with retained reports.

Both live fixture runners execute in Linux `cargo nextest`; the Python suite also checks retained evidence and rejects rehashed false PASS, fabricated Angular CRAP and reset debt. Hosted **Required Quality Aggregate remains CI pending at submission**; local validation is not its result.

Related records: [OpenSpec design](../../../../openspec/changes/dogfood-harness-gate-on-arc-admin/design.md), [tasks](../../../../openspec/changes/dogfood-harness-gate-on-arc-admin/tasks.md), [ADR-0049](../../../adr/0049-project-owned-validation-extension-boundary.md), [ADR-0044](../../../adr/0044-trusted-quality-baselines.md).
