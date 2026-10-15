# Issue #32: opt-in local deterministic command evidence

Status: implemented; required implementation CI passed, 2026-10-15.
See [acceptance evidence](../../../docs/quality/issue-32/README.md) for the exact commit and limits.

Repeated verify can reuse retained collector evidence but previously always ran
ordinary external steps. Add a versioned declaration and a separate host opt-in
for authenticated local command evidence. The implementation contract, input
identity, trust boundary, limits, compatibility and rollback are specified in
[the user guide](../../../docs/step-cache.md).

Engineering Policy semantics are unchanged. Core retains requiredness,
thresholds, baseline/debt/ratchet, current final decisions and release authority.
No required gate, waiver, baseline, measurement series definition or signed
request/replay-ledger behavior is weakened. #23 and #25 are already closed.

The first storage implementation is Unix-only. Unsupported hosts execute normally.
Source identity is conservative; v1 does not assume relocation equivalence for
separately materialized staged roots or restore missing outputs. Commands with
external state must not declare deterministic, side-effect-free caching.

The production coverage inventory includes every new cache source and excludes
only the cfg(test) test module. Risk selection gh32-step-cache/1 preserves prior
sources and adds cache identity/storage and task/scheduler integration. Both base
and head must be freshly measured under that identical expanded selection with
the existing pinned analyzer, metric definitions and thresholds. Historical
reports and debt baselines remain unchanged. This selection review changes the
self-measurement inventory, not product policy. Rollback restores the earlier
inventory and selection together; mixed-selection comparisons remain forbidden.

No new required CI benchmark job is introduced. Functional regressions run in
the existing test suite. Performance evidence is an optional local experiment,
with its own explicit synthetic identity and no cross-series acceptance claim.
