## Why this fails

An environment placeholder cannot be resolved or violates the supported interpolation syntax. The diagnostic points to the placeholder requiring attention.

## Diagnose and repair

Inspect the reported location and inline help. Supply required environment inputs or correct the placeholder syntax without exposing secret values.

Use the original diagnostic path, location and related entries to identify the
configuration to change. Run `harness-gate config check` after the repair; a
corrected field alone does not prove the whole workflow is valid.

## Example

When a required variable is absent in CI but present locally, configure the required input through the appropriate environment or secret mechanism. Do not substitute an empty string and assume the resulting configuration is equivalent.
