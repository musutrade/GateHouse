# GH-206 shared-service remediation

The future-concurrency preflight previously rejected shared consumers even when the scheduler
was configured to dispatch one step at a time. Resource diagnostics now honor
that serial execution contract (`parallel = false`, or a single configured worker).
[ADR-0050](../../../adr/0050-serial-shared-service-ordering.md) records this refinement of ADR-0026.
Parallel unordered consumers remain rejected. Dependency ordering, duplicate logs,
service injection conflicts and all quality thresholds remain enforced.

No imported step dependencies, commands, profiles or requiredness were changed.
Regression coverage loads both serial modes, retains parallel rejection and
executes two shared-service consumers exactly once in declaration order.
Resource validation is decomposed by responsibility and added to the versioned
base/head risk inventory; both revisions use the same measurement selection.

Arc-Admin native collector provisioning and authenticated baseline inputs are
separate integration prerequisites. Measurements must retain their actual failed
outcomes; neither diagnostic execution nor receipt collection authorizes authority
transfer or constitutes complete generic-quality acceptance.

## Local verification

The committed production repair `e6de28d` was measured against main `ba69cd5`
using `Collector.risk()` with selection `gh206-serial-resource-validation/1`.
Base coverage ran 388 tests and head coverage ran 390 tests, all passing with
none skipped. All 973 function identities passed the risk comparison. No risk,
coverage, profile or requiredness threshold changed. Raw manifests, source
archives, LLVM exports, counters and command logs are retained locally under
`/mnt/dev-ssd/dev-tmp/gh206-risk-verified/`.

Configuration regressions (55 tests), serial execution regressions (2 tests),
source measurement certification (13 tests), CI policy (12 tests), the historical
cost ledger (1 test), and self-hosted receipt handling and provenance (3 tests) passed. Clippy and
documentation consistency passed. These suites overlap with the full coverage
run and are not additive claims of unique tests.

Appending the new execution regression changes its source file hash. The five
existing critical-path bindings to that file were refreshed; their test symbols,
line ranges, assertions, source probes and platform applicability remain unchanged.
The complete critical-path inventory validates against the current sources.

The bounded measurement workflow runs pinned Arc-Admin commands on a temporary
self-hosted runner registered to Harness-Gate. It does not modify or replace
Arc-Admin's original multijob CI workflow. It records three sequential paired
full-workload trials with setup excluded and caches explicitly described. Those
results must not be presented as original-topology savings or complete-quality
cost when required collector/baseline provisioning still fails.
