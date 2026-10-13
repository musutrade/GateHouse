# GH-206 bounded self-hosted cost measurements

> Historical receipt: this page records the dated experiment below. For current native integration, released fixes and scoped cost observations, see the [2026-09-13 acceptance record](../../../quality/arc-native-20260913/README.md) and [release status](../../../release-status.md). Historical failures are preserved.

[Actions run](https://github.com/musutrade/Harness-Gate/actions/runs/34454586298) and [job](https://github.com/musutrade/Harness-Gate/actions/runs/34454586298/job/102797907379) measured Arc-Admin
`9982ed556eaf997910824d7b682946147c81a16a` using Harness-Gate `19a264e2b1992a5355166ca46e87fba766fa30d8` on the temporary
`gh206-cost-temporary` runner. [Derived report](selfhosted-report.json),
[raw artifact manifest](selfhosted/sha256.json) and
[download comparison](selfhosted/controller-comparison.json) retain the evidence.
All 753 downloaded artifact files matched the runner bytes. Source files and
staged configuration hashes remained unchanged. The ephemeral runner unregistered
after this bounded job; it is not a permanent duplicate CI lane.

| Pair | Before segment seconds | Shadow segment seconds | Added segment seconds |
| --- | --- | --- | --- |
| 1 | 399.29 | 546.24 | 146.95 |
| 2 | 293.08 | 539.66 | 246.59 |
| 3 | 293.65 | 536.46 | 242.81 |

Before runs `cargo flow verify --profile full --all`. Shadow runs that same
command once followed by the quality-composed Harness-Gate full workflow once.
Each of the three shadow invocations executed all 25 project-owned steps and
both prelude checks successfully, matching its paired traditional results.
The shared-service pre-dispatch failure is resolved by [ADR-0050](../../../adr/0050-serial-shared-service-ordering.md)
and the [generic repair](runtime-remediation.md), without editing the imported
flow, adding dependencies, changing profiles or deleting gates.

Every Harness-Gate invocation still exits 1 with `QUALITY_BLOCKED`: the trusted
`.harness-gate/runtime/full-state.json` is absent. Native collector/state/key and
baseline provisioning remains a GH-207 integration prerequisite. This is a real
failed quality outcome, not a successful full-quality trial. No native quality
measurement was produced, no accepted baseline changed, and no authority transfer
is authorized. `Quality profile null` in that early-failure diagnostic is also
retained as a follow-up UX gap rather than rewritten in the raw logs.

## Cost scope and limits

The single job occupied the runner for 3010 seconds,
with 4 seconds from run creation to job start.
GitHub step timestamps independently bracket all six workload segments. Command
receipts use a monotonic clock; segment totals also include report copying.
Job setup, compilation of Harness-Gate, checkout, npm installation and artifact
upload are outside the segment table and included in total job occupancy.
Runner-work here means occupied runner wall time, not CPU time or money.

This is a Harness-Gate-hosted measurement workflow executing pinned Arc-Admin
workloads on a real self-hosted runner. It does not reproduce Arc-Admin's original
parallel multijob topology, event routing, setup or separate security jobs; those
remain required in the proposed transfer design. The historical before workflow
costs in [the original ledger](report.json) are not subtracted from these samples.

The first pair includes initial-use compilation. All pairs share workspace-local
caches; pairs 2 and 3 observe the warmed sequence, not independent cold trials.
Their added segment times range from 242.81 to 246.59 seconds. This small ordered
sample is not a steady-state savings claim. Complete-quality cost and target
savings remain unknown until trusted native producers/baselines are provisioned
and the original-topology comparison is run. The [historical ledger](report.json)
retains its original unknown values; these new receipts are a separate series.

## Reproduce

Run `python3 docs/dogfood/arc-admin/cost/ci_reproduce.py` from the repository root.
It verifies the complete file manifest, Actions run/attempt/head/runner identity,
pinned Arc source, all sample receipts, elapsed calculations and the 27-check
runtime comparison. Tampered receipts and mixed CI identities fail validation.
The measurement workflow's successful conclusion means receipt collection
completed; it does not turn the retained quality failures into passes.
