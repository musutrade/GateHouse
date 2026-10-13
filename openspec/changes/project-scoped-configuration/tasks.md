# Tasks: Project-Scoped Configuration

**Parent:** [proposal.md](proposal.md), [design.md], and
[ADR-0029](../../../docs/adr/0029-project-scoped-configuration.md)
**Status:** Implemented; PR #38 merged and all required checks passed.

- [x] **1.1 (P0, S)** Remove process-environment project and workflow selectors.
  **Acceptance:** Discovery and config-path resolution use only explicit
  arguments or current-directory discovery; focused unit/CLI tests pass.
- [x] **1.2 (P0, S)** Remove audit-specific environment overrides and reject
  audit-path interpolation. **Acceptance:** `paths.audit_config` is literal,
  repository-relative, and has a typed diagnostic for `${...}` input.
- [x] **1.3 (P0, S)** Remove process working-directory mutation from project
  preparation. **Acceptance:** preparation preserves `current_dir` and report
  directory creation still succeeds.
- [x] **1.4 (P0, M)** Add Rust/Python isolation regression coverage and update
  docs. **Acceptance:** each project reads its own audit report despite the
  other project's inherited selector variables; ADR and configuration docs
  describe explicit migration paths.
- [x] **1.5 (P0, S)** Complete compatibility review and CI verification.
  **Acceptance:** formatter, linter, focused tests, documentation consistency,
  and all required PR checks are green; known local fixture failures remain
  clearly identified as unrelated user working-tree changes.

## Evidence

- Implementation: [PR #38](https://github.com/musutrade/Harness-Gate/pull/38)
- Green cross-platform and quality CI: [run 33217867592](https://github.com/musutrade/Harness-Gate/actions/runs/33217867592)
