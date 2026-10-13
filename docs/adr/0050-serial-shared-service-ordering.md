# ADR-0050: Treat guaranteed serial dispatch as shared-service ordering

## Status

Proposed with GH-206. Refines only the same-service resource preflight in
[ADR-0026](0026-configuration-safety-diagnostics.md).

## Context

ADR-0026 conservatively checked future concurrency from the dependency graph,
even while execution defaulted to serial. Arc-Admin's imported configuration
preserves implicit sequential consumers of one PostgreSQL service. That valid
source workflow was therefore rejected before Harness-Gate could execute it.
Adding dependencies solely to suppress the diagnostic could also change scope
selection by pulling otherwise unselected consumers into a plan.

## Decision

For the same service identity, guaranteed single-worker dispatch is sufficient
ordering: `execution.parallel = false`, or `parallel = true` with
`max_parallel = 1`. The current scheduler dispatches at most one plan node in
these modes. The production loader permits their shared consumers without
inserting dependencies, changing profiles or changing the execution plan.

When parallel execution permits more than one worker, the original dependency
reachability check remains mandatory across all configured steps, including
steps in different profiles. Unordered same-service consumers still produce
`HGCFG-SHARED-SERVICE`. Switching execution mode requires loading and validating
the new effective configuration; future CLI overrides or schedulers must not
widen concurrency after this validation without revalidating the same policy.
Invalid worker limits still fail their existing validation.

Duplicate logs remain errors in every mode. Distinct-service injection
collisions, per-step injection conflicts, service value protections, runtime
leases, cleanup, timeouts and fail-closed aggregation are unchanged. The serial
exception does not broadly suppress resource diagnostics or treat different
profiles as proof of mutual exclusion.

## Validation and consequences

Configuration regressions cover default serial dispatch, serial mode with an
otherwise larger worker limit, explicit one-worker parallel dispatch, parallel
rejection, ordered parallel reuse and duplicate-log rejection. A production-loader
and runtime regression executes two shared-service consumers exactly once in
declaration order. Arc-Admin's actual shared PostgreSQL consumers also execute
in the bounded self-hosted trials recorded by [GH-206](../dogfood/arc-admin/cost/runtime-remediation.md).

No project-owned command, profile, requiredness or quality threshold changes.
This decision addresses the shared-service compatibility gap, not trusted native
collector or baseline provisioning, and does not authorize authority transfer.
