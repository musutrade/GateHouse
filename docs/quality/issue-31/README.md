# Issue #31 implementation and local acceptance

The optional `scope.impact` version-1 contract declares consumer → dependencies
inside flow schema v2. Core computes reverse transitive impact before profile
filtering. Step prerequisites, explicit component overrides, unmatched policy and
mandatory built-in gates keep their existing meanings. No language/build-system
parser or executable graph-provider protocol is introduced.

The complete component inventory is mandatory. Unknown endpoints/versions,
missing/stale nodes, duplicate edges, cycles and budgets fail closed with
HGCFG-IMPACT diagnostics. Optional generated-graph source pins use exact SHA-256;
missing/stale/unsafe/oversized inputs fail before step dispatch. Read-only staged
plans and scope inspect indexed inputs; unstaged edits do not invalidate them.
Projects own graph truth and the completeness of source pins. No graph response
from a plugin is automatically adopted or trusted. An unsigned declaration is
ordinary reviewed project configuration, not authenticated plugin evidence.

Verification plans expose a deterministic shortest full witness chain and the
original changed paths/rule indices, plus all declared dependency edges. This
keeps diamonds and multiple roots explainable without enumerating exponentially
many chains. Explicit/all/unmatched-fallback reasons remain distinct.

Local validation: 535 nextest cases passed, zero skipped. Focused CLI regressions
cover working/staged/base edit, rename, delete and clean trees; actual dispatched
nodes equal the plan. Explicit/all behavior, profiles, invalid/stale graph data,
input fingerprints and filtered indexed inputs have negative regressions.
Additional staged-scope coverage confirms indexed pins ignore unstaged edits.
Formatting, Clippy with warnings denied, full documentation/schema consistency
and targeted measurement/instrumentation contracts passed.

The deterministic 1000-component/999-edge chain exercise performs 100 selections
in 876098 microseconds on this machine's debug build under concurrent regression
load (about 8.76 ms per selection). This is a local diagnostic, not a latency SLA
or a cross-platform certification. A leaf change selects one component. Graph
traversal visits reachable vertices/edges once with ordered-map lookup overhead;
execution cost is only the selected consumers' profile steps and prerequisite
closure. No subprocess is introduced for unpinned graph traversal.

Measurement review adds exactly config/impact.rs to the existing production and
AST/LLVM inventories, retaining every previous source, hotspot, analyzer,
threshold and required path. Both base/head must be recollected under the same
new selection identity gh31-component-impact/1. A new mandatory three-platform
scope.component_impact path binds the actual expansion to the thousand-component
regression. Existing critical-path bindings are refreshed for shifted source
locations/file digests without changing their observables or platform scope.
Engineering Policy authority, CRAP/coverage, baseline/debt/ratchet, waivers and
release decisions are unchanged. Applicable hosted coverage/risk/critical-path
gates remain required before merge; local functional success is not their
substitute. No baseline is reset and no historical evidence is overwritten.

Migration and conservative rollback are described in the
[design](../../../openspec/changes/component-impact-propagation/design.md).
