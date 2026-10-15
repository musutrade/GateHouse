## Why this fails

A configured audit or secret-scan configuration file is missing. A workflow file alone does not satisfy the required security inputs.

## Diagnose and repair

Create the audit or secret-scan configuration file named by the diagnostic path, or correct its configured path; do not bypass the required check.

Use the original diagnostic path, location and related entries to identify the
configuration to change. Run `harness-gate config check` after the repair; a
corrected field alone does not prove the whole workflow is valid.

## Example

If paths.audit_config points to a missing file, restore the intended reviewed audit policy or correct that path. Do not create an empty placeholder or disable the gate merely to satisfy a file-existence check.
