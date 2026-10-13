# GH-97 acceptance index

Measured implementation: `381d5f418655969d29b81f89069845e6667f35c9`.
Scope is final verification tasks 9.1–9.4. Implementation issues #88–#96 are
closed/completed; this evidence update does not accept the entire change.

| Task | Acceptance | Evidence and limits |
| --- | --- | --- |
| 9.1 | Accepted locally | Locked nextest: 315 passed, zero skipped; fmt and Clippy exit 0 at the measured SHA |
| 9.2 | Accepted locally | 119 quality and 23 release tests; both CLI contract modes 20/20; docs/schema; all four extension stages pass; negative CLI fixtures assert nonzero exits |
| 9.3 | Open | Linux evidence is current; retained macOS/Windows contracts are from `c0e612351c2efd515fde96bcefad80c1365534a2`; that run has an unresolved macOS test/benchmark failure. Platform policy reviewed; no fake-runtime result is claimed as real-container success |
| 9.4 | Evidence bookkeeping accepted | Status and task checkoffs match the retained evidence; strict OpenSpec validation passes; 9.3 and full-change acceptance remain open |

The [validation record](../../../docs/quality/gh-97-validation.md) links exact
commands, tool/target versions, raw artifacts, archived logs, hashes, hosted job
identities, platform applicability and environment failures. The raw command
ledger includes the final strict validation result. Evidence is attributed to
its measured SHA, not to later documentation commits.

Earlier acceptance records remain authoritative for their own scopes:
[GH-93](validation-gh-93.md), [GH-94](../../../docs/quality/gh-94-validation.md),
[GH-95](../../../docs/quality/gh-95-validation.md) and
[GH-96](../../../docs/quality/gh-96-validation.md).
[ADR-0039](../../../docs/adr/0039-required-risk-and-traceability-gates.md) governs
thresholds, exceptions, baseline acceptance and rollback. No baseline is accepted
by GH-97, and a successful PR aggregate cannot substitute for push-only platform
checks or resolve the recorded macOS failure.
