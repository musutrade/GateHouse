# Unified findings, SARIF and PR feedback

Issue: [GateHouse #33](https://github.com/musutrade/GateHouse/issues/33).
Contract and migration/rollback: [findings interface](../../../docs/findings.md).

Engineering Policy semantics are unchanged. This is a structured-result and
presentation boundary: Core keeps requiredness, threshold, debt/baseline/ratchet,
project decision and release authority. Tool severity cannot grant delivery or
publication authority. No analyzer algorithms enter Core.

The additive `harness-findings/1` bundle binds complete native/1 or SARIF 2.1.0
sources to exact completed Core report bytes and an immutable source tree.
Unknown/incomplete sources fail explicitly. Redacted source documents and
historical snapshots retain measurement IDs, tool versions and source pointers.
Explicit namespace/rule/anchor identity survives moves and line shifts; unstable
identity is diagnosed and cannot enter comparison. Pinned parent history preserves
resolved and reopened issues and refuses incompatible coverage or series.

An opt-in composite GitHub Action validates exact PR commit/tree, confined regular
file paths and blob/line identity, with paginated idempotency checks, 50-item
batches, bounded counts and stale-result cancellation. Publishing is separately
explicit; later history requires an independently trusted latest digest. Feedback
is advisory and cannot rewrite Core decisions. No new required collection stage
or network publication is added to this repository's CI.

Verification includes Rust boundary/negative tests, a real staged Core-report CLI
round trip and offline API fixtures for deduplication, pagination, redaction,
overflow, stale heads, forks, partial delivery and history enrollment. The existing
CI test job runs the offline Node fixtures; hosted release authorization remains
with the existing Required Quality Aggregate.

Risk measurement extends the selection to `gh33-findings/1` while retaining every
prior source. Both base and candidate must be freshly measured under that same
selection and existing pinned definitions. Historical series, thresholds and debt
remain unchanged; no prior report is relabeled as acceptance for this change.
