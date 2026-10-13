# ADR 0020: Decompose the Audit Scanner by Responsibility

## Status

**Accepted** - 2026-08-27

## Context

`audit/scanner.rs` combines repository-path resolution, regular-expression
compilation, source-line indexing, language-aware comment exclusion,
allowlist evaluation, traversal filtering, hard-rule scanning, and
architecture-rule scanning. The mixed concerns make the security and lexical
semantics difficult to change or review independently.

## Decision

After this design is accepted, split the scanner into private modules:

- `scanner/mod`: stable scanner exports and shared wiring.
- `scanner/paths`: rule-root and exclude-path resolution.
- `scanner/lexical`: source line lookup and comment-range detection.
- `scanner/filter`: regex compilation, allowlists, and traversal eligibility.
- `scanner/hard_rules`: hard-rule file scanning.
- `scanner/architecture`: architecture-rule scanning.

Preserve scanner inputs and outputs, repository containment, ignore-file
handling, symlink policy, path filtering, regex behavior, comment/string
semantics, allowlist behavior, traversal ordering, parallelism, violation
deduplication, line selection, and error text.

## Consequences

- Lexical comment handling and file eligibility become independently reviewable.
- Hard-rule and architecture-rule scanning have clear ownership.
- No child module becomes a public crate API.

## Related

- [ADR-0007](0007-audit-module-decomposition.md)
- [ADR-0018](0018-audit-boundary-decomposition.md)
- [OpenSpec: split-audit-scanner](../../openspec/changes/split-audit-scanner/proposal.md)
