## Why this fails

A required service lease could not be obtained or used safely. The accompanying lease error, not the code alone, identifies the failed condition.

## Diagnose and repair

Inspect the service setup log and required tools or credentials. Confirm resource ownership before cleanup, then retry the original verification.

Inspect the original failure detail and the affected step or artifact. Preserve
the failed evidence and rerun the intended operation after correcting its cause;
this explanation does not change retry policy or the final decision.

## Example

When a resource is still owned by another operation, inspect its recorded identity and lifecycle. Do not remove a lock just because the new verification wants the same name; resolve ownership before retrying.
