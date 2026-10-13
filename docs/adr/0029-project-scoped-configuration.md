# ADR-0029: Keep Workflow and Audit Configuration Project-Scoped

## Status

**Accepted** (2026-08-28)

## Context

Harness-Gate is installed once but is used from many repositories. A process
environment can be inherited by terminals, IDE tasks, hooks, and CI jobs that
operate on different repositories. Letting `PROJECT_ROOT`, a workflow-config
environment variable, or an audit-config environment variable select the
active project makes that shared process state part of verification policy.

That can select a different project's audit rules or make a valid project fail
because a foreign absolute path is rejected by repository-boundary validation.
Changing the process working directory during project preparation creates the
same kind of hidden coupling for callers that use multiple `Project` values in
one process.

## Decision

The active project is selected only by an explicit `--project-root` option or
by discovery from the process's current working directory. The workflow file
is selected only by an explicit `--config` option or the active project's
`.harness-gate/flow.toml`.

`paths.audit_config` in that workflow file is the sole selector for the audit
configuration. It is a literal repository-relative path and does not permit
environment interpolation. `PROJECT_ROOT`, `HARNESS_GATE_CONFIG`, `AUDITOR_CONFIG`, and
`HARNESS_GATE_AUDIT_CONFIG` are not configuration inputs. The configured audit
path remains repository-relative and is validated against the selected project
root.

Project preparation may create that project's report directories but must not
change the process working directory. All subprocess, Git, service, scan, and
report operations receive the project's root or resolved paths explicitly.

## Consequences

- Multiple repositories can be verified from a shared terminal, IDE, or CI
  environment without inheriting each other's audit configuration.
- Selecting a different project or workflow becomes explicit and observable in
  command invocations.
- Users relying on the removed environment-variable selectors must replace
  them with `--project-root` or `--config`.
- Existing report, secret-config, alias, service, and step environment inputs
  retain their documented behavior.
