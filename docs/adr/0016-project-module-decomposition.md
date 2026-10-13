# ADR 0016: Decompose the Project Module by Responsibility

## Status

**Accepted** - 2026-08-27

## Context

`tools/harness-gate/src/project.rs` combined project root discovery,
configuration loading, resolved project state, repository path containment,
runtime preparation, alias lookup and expansion, and path safety tests in one
246-line module. Discovery and runtime callers shared a useful `Project`
boundary, but the underlying responsibilities had different security and
maintenance concerns.

## Decision

Split project implementation into private submodules:

- `project/mod`: stable `Project` type and `resolve_repo_path` boundary.
- `project/discovery`: root discovery, configuration loading, project
  construction, and required-file validation.
- `project/paths`: repository-relative path validation, canonicalization, and
  symlink containment.
- `project/runtime`: report directory preparation, current-directory setup,
  alias lookup, and placeholder expansion.
- `project/tests`: project path safety tests.

Keep the existing `crate::project::Project` methods and
`crate::project::resolve_repo_path` path, environment variable behavior,
configuration resolution, error text, alias replacement order, filesystem
effects, and containment checks unchanged. Child modules remain private
implementation details.

## Consequences

- Security-sensitive repository path handling has a focused review boundary.
- Project discovery and runtime preparation can evolve independently.
- Existing domain modules retain the same project and path helper imports.
- Changes to resolved project fields may still require coordination across
  discovery and runtime modules.

## Alternatives Considered

- Leave the module monolithic: rejected because discovery, path security, and
  runtime preparation have distinct ownership and review risk.
- Expose separate public discovery and path types: rejected because this is a
  structural refactor and current callers benefit from the single `Project`
  boundary.
- Introduce a filesystem abstraction: rejected because it would expand the
  change beyond responsibility-based decomposition.

## Related

- [ADR-0015](0015-main-module-decomposition.md)
- [OpenSpec: split-project-module](../../openspec/changes/split-project-module/proposal.md)
