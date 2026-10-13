# ADR 0014: Decompose the Preset Module by Responsibility

## Status

**Accepted** - 2026-08-27

## Context

`tools/harness-gate/src/preset.rs` combined embedded preset definitions and
listing, project initialization, schema migration, atomic file writes, path
containment checks, and tests in one 281-line module. These responsibilities
have different change and security review concerns.

## Decision

Split preset implementation into private submodules:

- `preset/mod`: stable `init`, `migrate`, and `print_presets` exports.
- `preset/catalog`: embedded preset metadata and template constants.
- `preset/initialize`: new-project initialization and project ID generation.
- `preset/migration`: schema v1 migration workflow.
- `preset/filesystem`: atomic writes, overwrite policy, and path containment.
- `preset/tests`: preset, initialization, path, and write behavior tests.

Keep the existing `crate::preset::{init, migrate, print_presets}` boundary,
preset names and descriptions, generated paths and contents, output text,
overwrite rules, migration behavior, and path safety checks unchanged. Child
modules remain private implementation details.

## Consequences

- Security-sensitive filesystem operations have a focused implementation
  boundary.
- Initialization and migration workflows can evolve independently.
- Embedded templates remain centralized and easy to audit.
- Changes crossing initialization and filesystem policy still require review
  of both modules.

## Alternatives Considered

- Leave the module monolithic: rejected because catalog, workflows, and
  filesystem safety have distinct ownership.
- Create one module per embedded preset: rejected because preset data is
  declarative and belongs in one catalog.
- Replace atomic writes with a filesystem abstraction: rejected because this is
  a structural refactor and the current implementation is sufficient.

## Related

- [ADR-0013](0013-doctor-module-decomposition.md)
- [OpenSpec: split-preset-module](../../openspec/changes/split-preset-module/proposal.md)
