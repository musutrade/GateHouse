# Design: Project-Scoped Workflow and Audit Configuration

## Configuration Selection

Project discovery accepts only explicit function/CLI arguments for project
root and workflow config. With no override, it discovers from the process
working directory. The process environment is not consulted for
`PROJECT_ROOT` or `HARNESS_GATE_CONFIG`.

After the workflow is loaded, `paths.audit_config` is read from that file,
must be a literal repository-relative path, and is resolved against the
selected root. Audit-specific environment selectors are ignored and
interpolation in this field is rejected with a typed diagnostic. Other
configuration interpolation and documented field overrides retain their
existing behavior.

```text
explicit --project-root/--config
        or current-directory discovery
                    |
                    v
        selected project's flow.toml
                    |
                    v
 literal paths.audit_config -> resolve below project root -> audit runner
```

## Runtime Context

`Project::prepare()` creates the selected project's report log directory but
does not call `set_current_dir`. Subprocesses and adapters continue receiving
the project root or resolved paths explicitly, so two project values can be
prepared and executed in one process without shared working-directory state.

## Alternatives Considered

- Retain environment selectors with precedence: rejected because inherited
  terminal, IDE, or CI state can select another project's policy.
- Keep environment interpolation for `paths.audit_config`: rejected because
  it makes a security-sensitive selector depend on process-global state.
- Serialize projects by changing the process working directory: rejected
  because it remains unsafe for concurrent or nested callers.

## Rollback

Revert this change and restore the documented selectors only as one reviewed
compatibility decision. No configuration files or reports are rewritten by
the implementation, so rollback does not require data migration.

## Verification

- Unit test confirms preparation preserves the process working directory.
- CLI test runs independent Rust and Python projects while injecting each
  other's former selector variables.
- Loader test rejects `${...}` in `paths.audit_config` with a stable field
  path and diagnostic ID.
