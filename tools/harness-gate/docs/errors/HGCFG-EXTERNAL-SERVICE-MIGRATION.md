## Why this fails

The configuration uses the removed built-in external service policy shape. Its migration requires the supported external recipe format.

## Diagnose and repair

Use the GateProbes recipe migration to create a new configuration, retain the original for rollback, and run config check before verification.

Use the original diagnostic path, location and related entries to identify the
configuration to change. Run `harness-gate config check` after the repair; a
corrected field alone does not prove the whole workflow is valid.

## Example

Keep the original configuration, generate a separate migrated file using the documented GateProbes recipe migration, and review the diff. Check the migrated configuration before switching verification to it; retain the original for rollback.
