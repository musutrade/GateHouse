# Proposal: Keep Verification Configuration Project-Scoped

**Status:** Implemented
**Date:** 2026-08-28
**Change type:** Configuration isolation and runtime safety

## Goals

- Ensure a command run for project A cannot select project B's workflow or
  audit policy through inherited process environment variables.
- Keep workflow and audit paths explicit, repository-relative, and associated
  with the discovered project root.
- Allow multiple `Project` values in one process without changing its working
  directory.
- Provide Rust/Python cross-project regression evidence and document the
  migration from removed selectors.

## Non-goals

- Do not remove supported environment overrides for reports, secrets,
  aliases, service images, or step timeouts.
- Do not change audit rule syntax, report formats, project discovery commands,
  or the explicit `--project-root`/`--config` options.
- Do not make audit configuration global, user-wide, or dependent on the
  caller's working-directory mutations.

## Success Metrics

| Area | Success criterion |
| --- | --- |
| Isolation | `PROJECT_ROOT`, `HARNESS_GATE_CONFIG`, `AUDITOR_CONFIG`, and `HARNESS_GATE_AUDIT_CONFIG` cannot select the active project or audit file. |
| Audit path | `paths.audit_config` is a literal repository-relative path and is resolved below the selected project root. |
| Runtime | `Project::prepare()` leaves the process working directory unchanged. |
| Evidence | Rust and Python projects with different audit files pass when each command inherits the other project's old environment variables. |
| Compatibility | Explicit CLI selection and unrelated supported environment overrides retain their existing behavior. |

**Risk: Medium.** Existing users of removed environment selectors must move
to explicit CLI options; repository-boundary validation and focused tests limit
the risk of selecting an unintended policy.

## Related Records

- [ADR-0029: Keep Workflow and Audit Configuration Project-Scoped](../../../docs/adr/0029-project-scoped-configuration.md)
- [Configuration reference](../../../docs/configuration.md)

## Implementation Evidence

- Implementation: [PR #38](https://github.com/musutrade/Harness-Gate/pull/38)
- Green cross-platform and quality CI: [run 33217867592](https://github.com/musutrade/Harness-Gate/actions/runs/33217867592)
- Accepted decision: [ADR-0029](../../../docs/adr/0029-project-scoped-configuration.md)
