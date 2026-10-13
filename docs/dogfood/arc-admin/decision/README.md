# GH-208 authority-transfer recommendation

**Decision: NO TRANSFER. Retain `cargo flow` required workflow authority and
bounded Harness-Gate shadow mode.** This recommendation follows GH-207 against
Arc-Admin `9982ed556eaf997910824d7b682946147c81a16a`. Traditional full-workload
parity is demonstrated within the recorded conditions; native generic quality,
host/hook/CI parity and complete cost acceptance are not. Missing evidence is a
blocker, not permission to lower assurance. This record does not deploy changes
to Arc-Admin, accept the entire proposal or authorize infrastructure removal.

Harness-Gate Result remains validation evidence, not an application/workflow
lifecycle state source. It cannot approve a release, advance delivery state,
close an issue or replace the controller's merge and CI decisions. The released
Rust core retains generic quality policy authority; project-owned tests retain
their application semantics.

## Retained final evidence

These are the final available observations for this recommendation, not claims
that every migration activity is complete. Historical receipts remain intact.
[evidence-index.json](evidence-index.json) pins the retained source/report/manifest
hashes; the referenced manifests bind the raw artifacts.
[validation.json](validation.json) records this issue's actual local checks.

| Evidence | Supported conclusion and boundary |
| --- | --- |
| [Frozen baseline](../README.md), [inventory](../inventory.json) and [source manifest](../source-manifest.json) | Preserve all 25 selected-step blockers (including two outside the 23-entry policy list), both prelude checks, services, profiles, environment, scope, hooks and CI-only obligations. |
| [Migration report](../import/flow.import.json) and [effort record](../import/README.md) | One import command; zero manually re-entered steps or generated configuration edits; 491 preserved scalar values; 25 duplicated step definitions while both configurations are retained. Human elapsed migration time and remaining runtime integration effort are unmeasured. Execution-only import does not migrate global Arc overrides or establish workflow readiness. |
| [Quality binding](../quality/README.md) and [configuration](../quality/quality.toml) | Certified reference bindings exist; native producers, signed state, trusted keys and authenticated baseline lineage are not demonstrated. Angular/TypeScript CRAP remains unsupported. |
| [Historical shadow matrix](../shadow/README.md), [GH-207 rerun](../remediation/README.md), [receipt](../remediation/evidence/receipt.json) and [manifest](../remediation/manifest.json) | The original pre-dispatch limitation was repaired. The current three local full runs each pass 27 traditional results. Arc exits 0; both Harness-Gate paths exit 1, with quality blocked in configuration on missing `.harness-gate/runtime/full-state.json` and no project quality report. Selecting an alternate execution config still loads project-root quality. This is bounded command parity, not complete workflow parity. |
| [Controlled negatives](../negative/README.md), [GH-207 negative manifest](../remediation/negative/manifest.json) | Six CLI controls/negatives and seventeen signed synthetic quality cases retain expected outcomes, including required command failures, absent evidence, stale/incompatible baselines, CRAP regression, unsupported Angular CRAP and honest omissions. Synthetic evidence does not prove native Arc-Admin integration. |
| [Cost/ownership ledger](../cost/README.md), [historical report](../cost/report.json), [paired self-hosted series](../cost/selfhosted.md) and [derived report](../cost/selfhosted-report.json) | Historical CI: 799 seconds execution span, 1,077 runner-seconds. Three later paired segments add 146.95, 246.59 and 242.81 seconds; the bounded job occupies 3,010 runner-seconds. All shadow quality outcomes fail. Different topology, shared caches and initial compilation prevent treating these as target savings; complete-quality and original-topology costs remain unknown. |
| [Product-gap ledger](../remediation/README.md), [ADR-0050](../../../adr/0050-serial-shared-service-ordering.md) | Serial shared-service loading, static import validation and early quality-profile diagnostics were repaired with generic regressions. Those repaired gaps do not erase the remaining integration and evidence blockers below. |

## Exact blockers and evidence required to reconsider

[GH-215](https://github.com/musutrade/Harness-Gate/issues/215) tracks B1–B4.
Arc-Admin integration/CI maintainers own native producers, signing/provenance and
routing; Harness-Gate maintainers own any reproducible generic interface gaps.
Tracking a blocker is not resolving it.

| ID | Blocker | Required evidence for a new recommendation |
| --- | --- | --- |
| B1 | Native quality fails on missing `full-state.json`; native backend/frontend/API producers, fresh signed workflow inputs, trusted host keys and authenticated Git merge-base baseline lineage are unproven. | Provision the real configured producers and full/hook inputs. Retain native full-quality success with source/selection/series identity and authenticated baseline provenance, then native missing/tampered/stale evidence and baseline negatives. Preserve coverage/CRAP thresholds, debt, ratchets and fail-closed behavior. |
| B2 | Full/all command success does not prove hook staged/working-tree selection, global environment override migration, adversarial isolation, service failure/cleanup or destination scope/hook/CI routing. | Matched source/scope/profile success and failure trials covering each condition, including prelude and separate security/CI routes. Preserve every existing blocker, timeout, service/environment contract and requiredness. Keep honest NOT_RUN/NOT_COMPUTED/unsupported outcomes. Do not invent a `ci` profile. |
| B3 | Negative evidence exercises generic/synthetic boundaries; real integration fail-closed behavior and rollback operation are unproven. | Re-run native command/evidence/baseline/CRAP failures after B1/B2 integration and record a rollback rehearsal at the proposed destination. Existing synthetic regressions remain required evidence, not substitutes. |
| B4 | Complete-quality cost, original multijob topology comparison and target savings are unknown; no accepted final ownership deployment exists. | Paired self-hosted complete-quality trials and original-topology comparison, retaining setup/cache/order/job overhead, collection/parsing/report costs and duplication. Demonstrate one command owner and one authoritative producer per compatible supported measurement identity, with authenticated artifact reuse and evaluation-only aggregation. Accept the measured cost before transfer. |
| B5 | Proposal-wide acceptance is incomplete; policy/ADR tasks 1.1, 1.2 and 1.4 remain unchecked. | Complete and validate those separately scoped tasks, review all acceptance claims against evidence, and obtain Required Quality Aggregate green for the submitted change through the controller. Strict OpenSpec validation alone is not acceptance. |

## Rollback boundary and infrastructure hold

The current boundary is before any authority change: existing `cargo flow`, hooks,
required CI routes, implementation, configuration and report artifacts remain in
place. Preserve project commands and all existing checks; retain the configured
quality thresholds, baseline identity/lineage and debt/ratchet history. Continue
only bounded, identified shadow observations with visible duplicate cost.

If shadowing fails or exceeds its trial budget, stop the observation and retain
its failed evidence; continue the existing required workflow. Do not convert a
blocked quality result to PASS, fabricate native measurements, reset a baseline,
or remove an existing blocker. This is an operational boundary, not a claim that
a destination rollback drill has already succeeded.

Any later transfer needs a separately accepted change with pinned configurations,
tool versions and required-check routing; proven owners for all obligations;
retained old executable/configuration/CI routes; and an exercised restoration
procedure. Missing/invalid evidence, lost gates, routing/service differences or
unaccepted cost require restoring the previous required authority and stopping
the transfer. Retain failed/new artifacts and authenticated baseline history;
never rewrite lineage to make rollback green. Removing or consolidating old
infrastructure requires a separate follow-up after safe transfer is evidenced.
Task 9.3's safe-transfer condition is false, so no removal change is created here.

## Validation and closure disposition

Replay the retained evidence without an Arc-Admin checkout:

```bash
python3 docs/dogfood/arc-admin/remediation/reproduce.py
python3 docs/dogfood/arc-admin/cost/ci_reproduce.py
python3 -m unittest discover -s tools/quality/tests -v
openspec validate dogfood-harness-gate-on-arc-admin --strict
```

The Python suite also checks baseline, import, quality bindings, historical shadow,
negative and cost records and rejects tampered receipts. Local validation does
not establish hosted Required Quality Aggregate success. That remains CI pending
at submission and must be green before controller acceptance.

Tasks 9.1 and 9.2 are complete for the no-transfer outcome. Task 9.3 is conditional
and not applicable; 9.4 retains evidence and strict validation here, while closure
remains withheld until all proposal claims are supported. The OpenSpec remains
active, ADR-0049 remains proposed, and GH-208 remains open for controller handling.

Related: [OpenSpec design](../../../../openspec/changes/dogfood-harness-gate-on-arc-admin/design.md),
[tasks](../../../../openspec/changes/dogfood-harness-gate-on-arc-admin/tasks.md),
[ADR-0049](../../../adr/0049-project-owned-validation-extension-boundary.md),
[ADR-0040](../../../adr/0040-language-agnostic-evidence-policy.md),
[ADR-0044](../../../adr/0044-trusted-quality-baselines.md).
