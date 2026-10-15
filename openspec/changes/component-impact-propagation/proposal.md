# Explicit component impact propagation (#31)

A shared component change currently selects only its own steps. Declare a generic,
repository-owned component graph so Core can select its transitive consumers,
without importing any language's dependency discovery or build semantics.

Add optional `scope.impact` version 1 inside flow schema version 2. Consumer keys
map to dependencies; every workflow component appears, including empty lists.
Reject unsupported versions, unknown/missing/stale components, duplicate edges,
cycles and resource-budget violations. No graph preserves previous selection.
Repository/plugin authors produce and review configuration data; Core never
executes a graph provider or accepts an unauthenticated runtime graph response.

Optional SHA-256 input pins bind generated declarations to their source manifests
or lockfiles. Missing, unsafe, transformed staged, oversized or changed pinned
inputs block selection. Static declarations without pins are maintained by the
project; Core cannot prove undeclared import relationships. Generated graphs
must pin every graph-changing input and be reviewed before adoption.

Engineering Policy semantics remain unchanged: requiredness, thresholds,
baseline/debt/ratchet, aggregation and final release authority stay in Core.
Profiles, execution prerequisites, required built-ins and waivers are unchanged.
No previous measurement report is rewritten and no quality baseline is reset.
The additive risk inventory includes config/impact.rs and recollects both base
and head under gh31-component-impact/1, retaining all previous sources and tools.

See [design](design.md) and [tasks](tasks.md).
