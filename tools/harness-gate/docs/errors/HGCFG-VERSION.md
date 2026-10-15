## Why this fails

The workflow version is outside the supported configuration contract. A syntactically valid TOML file can still use an unsupported schema version.

## Diagnose and repair

migrate a v1 file with `harness-gate config migrate` or use version 2

Use the original diagnostic path, location and related entries to identify the
configuration to change. Run `harness-gate config check` after the repair; a
corrected field alone does not prove the whole workflow is valid.

## Example

For an existing version-1 workflow, use the documented config migrate command and inspect its changes before use. Merely replacing the version number does not migrate incompatible fields.
