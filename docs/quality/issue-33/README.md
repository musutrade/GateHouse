# Issue #33 validation

Contract: [findings, SARIF and explicit PR feedback](../../findings.md).
Change governance: [OpenSpec](../../../openspec/changes/unified-findings/README.md).

Engineering Policy semantics are unchanged. These findings and feedback are
advisory; no existing gate, threshold, waiver, baseline/debt rule or final Core
permission changes. The new production sources are included in the existing
blocking app coverage boundary. Native risk selection extends additively to
`gh33-findings/1`; both sides must be freshly collected with the same selection,
pinned analyzer and metric definitions. Existing evidence is not relabeled.

Local validation during implementation:

- The complete workspace nextest run passed **566/566 tests, zero skipped**.
  Subsequent focused tests cover the final history snapshot, URI and redaction
  refinements; the final hosted run must validate the complete candidate again.
- Fourteen finding unit regressions cover malformed/incomplete documents, provenance,
  native/SARIF import/export, URI decoding, path confinement, duplicate identities,
  cross-tool canonical mapping, moved lines/files, resolution/reopening, pinned
  history and immutable outputs. A 500-finding fixture bounds history serialization
  and prevents copying the complete result set into every record. A large-metadata
  fixture also proves source metadata is retained once; identifier bounds prevent
  large shared tool/measurement fields from expanding into every observation.
- A real CLI integration runs `verify --staged`, then normalize/compare/export
  and both schema generators. It checks retained Core report bytes, source tree
  binding, independently pinned parents, exclusive output creation, schema sync,
  and that tool error severity cannot replace the original Core PASS.
- Eleven offline Node tests exercise the actual action publisher with a fake API:
  explicit dry-run, exact commit/tree/blob/line binding, 50-item batches, paginated
  idempotency, deduplication with strongest severity, redaction, limits, stale/fork
  PRs, partial failure/cancellation and explicit initial history enrollment.
- Forty-one production-inventory, CI orchestration and risk-selection tests pass.
  The inventory test initially rejected a new boundary and an undeclared test
  file. The repair registers production under the existing blocking app boundary
  and declares only the cfg(test) source as test code; no assurance is dropped.
- The CLI contract snapshot differs only by the additive `findings` help row.
  That row was reviewed and the complete contract check subsequently passed.
- Documentation consistency, rustfmt and Clippy with all targets/features and
  warnings denied are required local checks. The pinned native AST analyzer
  accepts all four added production sources and closure instrumentation.

The first scope has no organization history service, dashboard, automatic fixes,
code-scanning upload or implicit posting workflow. Local tests have not published
annotations to a real PR; offline API fixtures are not evidence of real GitHub
annotation rendering. The action remains opt-in and requires trusted report and
history digests plus explicit write authorization.
