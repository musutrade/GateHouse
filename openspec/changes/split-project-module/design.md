# Design: Split the Project Module

## Module Layout

```text
src/project/mod.rs
  stable Project type and resolve_repo_path boundary
src/project/discovery.rs
  root/config discovery, project construction, required-file validation
src/project/paths.rs
  repository-relative validation, canonicalization, symlink containment
src/project/runtime.rs
  preparation, current-directory setup, alias lookup and expansion
src/project/tests.rs
  repository path safety tests
```

The parent keeps the existing `crate::project::{Project, resolve_repo_path}`
boundary. Child modules expose only the implementation needed by the parent or
sibling modules; no child module becomes part of the crate API.

## Behavior Preservation

The extraction must preserve:

- Project root override and environment-variable precedence.
- Configuration override resolution and ancestor discovery behavior.
- Resolved project fields and required configuration validation.
- Repository-relative path rejection, existence rules, canonicalization, and
  symlink containment.
- Report directory creation, current-directory changes, alias lookup, and
  placeholder replacement order.
- Existing method signatures, helper imports, error text, and test behavior.

## Verification

Run the complete nextest suite, format check, Clippy with all targets and
features, rustdoc, `git diff --check`, and strict OpenSpec validation. Inspect
the diff for changed discovery precedence, path checks, error strings,
filesystem effects, and alias replacement order.

## Rollback

Revert the single refactoring commit. No configuration or generated artifact
migration is required.
