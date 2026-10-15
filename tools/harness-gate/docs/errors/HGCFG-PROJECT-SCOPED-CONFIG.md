## Why this fails

The audit configuration path attempts to depend on environment interpolation. Security policy selection must remain bound to the project configuration.

## Diagnose and repair

Use a literal repository-relative audit configuration path. Do not redirect the security policy through environment interpolation.

Use the original diagnostic path, location and related entries to identify the
configuration to change. Run `harness-gate config check` after the repair; a
corrected field alone does not prove the whole workflow is valid.

## Example

Use a literal repository-relative audit path in flow.toml. If selecting a different project is intentional, use --project-root or --config explicitly rather than using an environment value to redirect the audit policy.
