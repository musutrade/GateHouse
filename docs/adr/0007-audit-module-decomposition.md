# ADR 0007: Decompose the Audit Module by Responsibility

## Status

**Accepted** - 2026-08-27

## Context

`src/audit.rs` has grown into the largest production module in Harness Gate.
It currently owns audit configuration parsing and validation, source scanning,
comment-range detection, report serialization, and structured-log parsing.
These responsibilities change independently, which makes local reasoning and
focused testing harder than necessary.

## Decision

Split the audit implementation into private submodules organized around
responsibility:

- `audit/config`: schema types, parsing, and configuration validation.
- `audit/scanner`: source traversal, lexical comment handling, and rule scans.
- `audit/report`: JSON and Markdown report models and rendering.
- `audit/log_parser`: structured-log trace selection and context extraction.

Keep `audit::run`, `audit::parse_logs`, `AuditError`, and `AuditOutcome` at the
existing module boundary. The submodules remain private implementation details;
their function signatures, report formats, configuration schema, and error
codes are not changed.

## Consequences

- Each audit concern has a smaller file and a clear owner.
- Existing unit and integration tests continue to exercise the same public
  command paths without API changes.
- Cross-cutting changes may require updating the narrow internal interfaces
  between the submodules.
- Rust module visibility becomes an explicit guard against accidental coupling.

## Alternatives Considered

- Leave the monolithic module in place: rejected because the file size and
  mixed responsibilities already slow maintenance.
- Split by individual function: rejected because it would create many tiny
  modules without meaningful ownership boundaries.
- Extract a public audit library: rejected because Harness Gate is a binary
  crate and these implementation types are not an external API.

## Related

- [ADR-0006](0006-test-fixtures-and-internal-boundaries.md)
- [OpenSpec: split-large-modules](../../openspec/changes/split-large-modules/proposal.md)
