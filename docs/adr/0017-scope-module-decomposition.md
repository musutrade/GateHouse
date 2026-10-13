# ADR 0017: Decompose the Scope Module by Responsibility

## Status

**Accepted** - 2026-08-27

## Context

`tools/harness-gate/src/scope.rs` combined scope error codes, selection mode
and result models, Git change detection, configuration classification, report
serialization, and tests in one 227-line module. These responsibilities have
different review concerns, especially Git input handling and report output.

## Decision

Split scope implementation into private submodules:

- `scope/mod`: stable `ScopeError`, `ScopeMode`, `ScopeResult`, and `detect`
  exports.
- `scope/errors`: typed scope errors and error-code mapping.
- `scope/model`: selection modes and scope result construction.
- `scope/detection`: Git worktree checks, changed-path collection, and scope
  classification.
- `scope/report`: changed-file and JSON report writing.
- `scope/tests`: scope classification tests.

Keep the existing `crate::scope::{ScopeError, ScopeMode, ScopeResult, detect}`
boundary, Git command selection, path de-duplication, base-reference handling,
classification rules, unmatched-file policy, report contents, error text,
error codes, and output ordering unchanged. Child modules remain private
implementation details.

## Consequences

- Git scope detection and configuration classification have a focused review
  boundary.
- Report serialization changes can be reviewed separately from selection logic.
- Existing callers retain the same scope types and detection function.
- Changes to scope behavior may still require coordination across detection,
  model, and report modules.

## Alternatives Considered

- Leave the module monolithic: rejected because Git detection, policy
  classification, and reporting have distinct ownership and failure modes.
- Introduce a generic scope-provider trait: rejected because the binary crate
  has one Git-backed implementation and no extension API requirement.
- Merge errors into the crate-wide error module: rejected because scope error
  codes and messages are domain-specific and already form a stable boundary.

## Related

- [ADR-0016](0016-project-module-decomposition.md)
- [OpenSpec: split-scope-module](../../openspec/changes/split-scope-module/proposal.md)
