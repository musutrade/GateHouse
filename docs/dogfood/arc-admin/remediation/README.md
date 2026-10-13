# GH-207 product gap remediation

> Historical receipt: this page records the dated experiment below. For current native integration, released fixes and scoped cost observations, see the [2026-09-13 acceptance record](../../../quality/arc-native-20260913/README.md) and [release status](../../../release-status.md). Historical failures are preserved.

This record implements OpenSpec tasks 8.1–8.4. It preserves the historical
GH-204/205/206 receipts and the frozen Arc-Admin commands, requiredness, profiles,
policies and measurement series. Native quality integration remains blocked and
is separately tracked in [GH-215](https://github.com/musutrade/Harness-Gate/issues/215).
Task 9 and acceptance of the whole proposal remain pending.

| Discrepancy / source | Disposition and rationale | Evidence / closure |
| --- | --- | --- |
| HG-CAP-001: serial shared-service consumers rejected (GH-204) | Fixed in GH-206 by recognizing guaranteed single-worker dispatch. No dependencies were added to Arc-Admin. | [ADR-0050](../../../adr/0050-serial-shared-service-ordering.md), existing generic serial load/execution and parallel rejection tests; current unchanged-flow rerun below. |
| HG-UX-001: import success followed by production configuration rejection (GH-204) | Fixed: offline import now uses the loader's static semantic/resource validator before publishing output. Host overrides remain excluded. Explicit readiness stages below distinguish static checks from trusted runtime evidence. | Generic duplicate-log import rejection and valid serial-flow acceptance regression; frozen import bytes unchanged; both imported configurations pass production `config check` and `scope --all`. |
| HG-UX-002: early quality failure says `Quality profile null` (GH-206) | Fixed: the requested profile is retained even if preparation fails. Status/phase remain blocked/configuration, with no fabricated policies or project report. | Generic missing-state and missing-key CLI regressions; current full workflow retains `Quality profile "full": blocked`. |
| Alternate `--config` still discovers project-root quality (GH-207 rerun) | Expected generic project-wide configuration semantics, clarified below: selecting an execution flow does not disable `.harness-gate/quality.toml`. | Both flow-path reruns preserve all 27 command results and the same missing-state block; semantic receipt checks reject promoting either overall result to PASS. |
| Required native collectors, workflow state, trusted keys and authenticated baseline are absent (GH-203/204/206) | Expected stricter generic-quality difference, with project/host integration work tracked in GH-215. Reference packs and synthetic signed fixtures cannot supply native measurements or trusted lineage. | GH-215 requires real producer/state/key/baseline provisioning and native success plus negative evidence. Missing `full-state.json` continues to fail closed. |
| Hook staged/working-tree semantics, global environment override migration, adversarial isolation/service lifecycle and destination scope/CI routing are not fully demonstrated (GH-202/204) | Explicitly tracked in GH-215. Successful full workloads establish a bounded observation; they do not prove all hook/host/failure conditions. Global Arc overrides remain intentionally unmigrated. | Matched hook/host scenarios and routing evidence required; original hooks, CI routes and all application gates stay required. |
| Complete-quality cost and original-topology savings are unknown (GH-206) | Tracked in GH-215. The three real self-hosted pairs measure sequential project-command workloads with failed quality, not native collection or the original multijob topology. | [Bounded cost series](../cost/selfhosted.md); new local runs are a separate series. Complete-quality and original-topology measurements remain required. |
| Required command/evidence failures, stale/incompatible baselines, Rust CRAP regressions, unsupported Angular CRAP and profile omissions (GH-205) | No additional product discrepancy: the controlled cases retain their specified failures/omissions. Project E2E/API/smoke logic remains project-owned under [ADR-0049](../../../adr/0049-project-owned-validation-extension-boundary.md). | Rerun six CLI controls/negatives and seventeen signed synthetic quality cases; no framework-specific Generic Core branch. |

## Current rerun scope

Both Harness-Gate flow paths run in the same prepared project, which also contains
`.harness-gate/quality.toml`. Quality discovery is project-wide: `--config` selects
the execution flow and does not disable conventional quality discovery. Therefore
`execution` in the receipts identifies the execution-import **flow path**, not a
quality-disabled workflow control. Both Harness-Gate runs must fail closed on the
same missing native state after their traditional command results are compared.
This behavior is now explicit in the migration procedure; it is not a reason to
remove the quality requirement. These local runs are a separate observation from
the historical self-hosted cost series and cannot establish native-quality parity.

The retained rerun completed on 2026-09-10 against Arc-Admin
`9982ed556eaf997910824d7b682946147c81a16a`, with unchanged tracked source and
configuration hashes. [Command receipts](evidence/receipt.json) and the
[artifact manifest](manifest.json) bind these observations.

| Flow | Application results | Overall exit | Full-run elapsed |
| --- | --- | --- | --- |
| Arc-Flow | 27 passed | 0 | 459.617 s |
| Harness-Gate execution import path | 27 passed | 1: native quality blocked | 319.759 s |
| Harness-Gate quality flow path | 27 passed | 1: native quality blocked | 262.015 s |

All six controlled CLI cases and seventeen signed synthetic quality cases met
their expected outcomes. The semantic replay also checks scope, command identity,
profile identity and fail-closed status; regression mutations reject missing
commands, false overall success and lost profiles. The local function-risk
comparison passed for 973 identities with zero failures. These timings are raw
local observations, not a new cost acceptance claim.

## Migration readiness

1. `config import --execution-only` checks supported, lossless declarations and
   static semantic/resource constraints without executing commands or reading
   environment values. It rejects duplicate log destinations before writing
   either output file. Its success is an execution-config import, with authority
   transfer still blocked.
2. Run `harness-gate --project-root <project> --config <flow> config check` and
   `scope --all` in the intended host environment. These exercise production
   loading and scope. Quality binding validation checks contracts, not native
   producer availability or signed runtime inputs.
3. Run the selected workflow on matched source/scope to establish actual command
   and quality outcomes. A successful static check cannot turn missing trusted
   state/keys/baselines into runtime PASS. Use the reported profile, phase and
   error to identify the failing stage. GH-215 owns the missing integration.

## Reproduce

From this repository, with a built `target/debug/harness-gate`:

```bash
python3 docs/dogfood/arc-admin/cost/selfhost_trial.py prepare --output target/quality/gh-207/parity
python3 docs/dogfood/arc-admin/remediation/run.py --prepared target/quality/gh-207/parity --harness-gate target/debug/harness-gate
python3 docs/dogfood/arc-admin/negative/run.py --harness-gate target/debug/harness-gate --output target/quality/gh-207/negative/evidence
HARNESS_GATE_DOGFOOD_NEGATIVES="$PWD/target/quality/gh-207/negative/evidence" cargo nextest run --manifest-path tools/harness-gate/Cargo.toml --locked -E 'test(arc_admin_controlled_quality)'
python3 docs/dogfood/arc-admin/remediation/reproduce.py
```

Choose a fresh workspace-local `target/` directory for another live trial.
Preparation fetches the frozen Arc-Admin commit into that disposable directory;
it does not access another workspace. Reports from each engine are copied before
the next engine runs, with stale report directories removed between invocations.
Source/configuration hashes are checked before and after. The final command
replays the committed, hashed receipts and checks their semantics; it does not
rerun the application or establish hosted CI success.

See [validation.json](validation.json) for actual commands and limitations.
Root `harness-gate config check` and `harness-gate verify --profile ci --all` are
not applicable: this source repository has no `.harness-gate/flow.toml` or declared
`ci` profile. The fixture checks above are separately identified. Required Quality
Aggregate is CI pending until the controller verifies the submitted commit.
