## Why this fails

Configuration checking failed outside a more specific typed diagnostic. The original error and project selection are needed to determine the actual cause.

## Diagnose and repair

Verify the project root, configuration path and required security configuration files; inspect the original command error.

Use the original diagnostic path, location and related entries to identify the
configuration to change. Run `harness-gate config check` after the repair; a
corrected field alone does not prove the whole workflow is valid.

## Example

If checking fails before a workflow can be loaded, verify the selected project root and configuration path. Do not treat this generic code as proof that the TOML syntax is wrong or overwrite the configuration with a guessed template.
