## Why this fails

The scheduler or its surrounding orchestration encountered an error. The underlying detail can concern scheduling, infrastructure or cleanup and must be inspected before deciding what to repair.

## Diagnose and repair

Inspect the first scheduler or cleanup failure and its step logs. Resolve the underlying error before rerunning verification.

Inspect the original failure detail and the affected step or artifact. Preserve
the failed evidence and rerun the intended operation after correcting its cause;
this explanation does not change retry policy or the final decision.

## Example

If preparation of a scheduled node fails, repair that first error and inspect which dependents were skipped. A skipped dependent has not supplied successful evidence, even if unrelated nodes passed.
