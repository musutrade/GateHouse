# ADR 0018: Complete the Audit Boundary Decomposition

## Status

**Accepted** - 2026-08-27

## Context

After ADR-0007 separated audit configuration, scanning, report generation,
and log parsing, `tools/harness-gate/src/audit/mod.rs` still combined the
public boundary, typed errors, result and violation models, run orchestration,
and the audit test suite in a 738-line module. This obscured the production
entry points and kept test-only dependencies near runtime behavior.

## Decision

Complete the audit decomposition with private submodules:

- `audit/mod`: module wiring and stable `run`, `parse_logs`, `AuditError`, and
  `AuditOutcome` exports.
- `audit/errors`: typed audit errors and error-code mapping.
- `audit/model`: audit result and internal violation models.
- `audit/runner`: configuration loading, scan orchestration, and report output.
- `audit/tests`: audit boundary, configuration, scanner, and log-parser tests.

Keep the existing public audit API, error text and codes, configuration load
order, scan ordering, report JSON and Markdown output, truncation behavior,
JSON emission, and log parsing behavior unchanged. Existing configuration,
scanner, report, and log-parser modules remain unchanged.

## Consequences

- Runtime entry points are small and easy to review.
- Error mapping and output orchestration have focused ownership boundaries.
- Test-only imports no longer inflate the production module.
- Runner changes still require careful review of report and scanner contracts.

## Alternatives Considered

- Leave `audit/mod.rs` as a mixed boundary: rejected because it remains the
  largest source file after the first audit decomposition.
- Split every audit test by feature: rejected because the existing suite is
  cohesive boundary coverage and one dedicated test module is sufficient.
- Change the audit public API: rejected because this is structural refactoring
  with no user-facing behavior change.

## Related

- [ADR-0007](0007-audit-module-decomposition.md)
- [ADR-0017](0017-scope-module-decomposition.md)
- [OpenSpec: complete-audit-module-decomposition](../../openspec/changes/complete-audit-module-decomposition/proposal.md)
