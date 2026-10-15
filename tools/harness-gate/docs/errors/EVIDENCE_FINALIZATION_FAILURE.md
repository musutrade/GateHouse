## Why this fails

Writing or verifying the final evidence registry, manifest or machine result failed. The detailed error identifies the failing finalization operation.

## Diagnose and repair

Inspect the affected artifact binding and invocation identity. Correct the producer or path and generate fresh evidence; do not edit manifests to make stale or missing evidence pass.

Inspect the original failure detail and the affected step or artifact. Preserve
the failed evidence and rerun the intended operation after correcting its cause;
this explanation does not change retry policy or the final decision.

## Example

If a manifest write fails, correct the output environment and rerun the required verification. Preserve the failed result for diagnosis instead of marking the incomplete evidence set finalized.
