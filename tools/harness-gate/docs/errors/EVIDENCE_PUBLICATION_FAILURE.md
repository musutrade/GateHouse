## Why this fails

A machine or human report publication operation failed. The retained failure detail identifies the affected output rather than proving all other outputs are absent.

## Diagnose and repair

Inspect the affected artifact binding and invocation identity. Correct the producer or path and generate fresh evidence; do not edit manifests to make stale or missing evidence pass.

Inspect the original failure detail and the affected step or artifact. Preserve
the failed evidence and rerun the intended operation after correcting its cause;
this explanation does not change retry policy or the final decision.

## Example

If a report cannot be atomically written to its destination, repair that destination and regenerate the required result. Do not use an older file at the same path as evidence that this publication succeeded.
