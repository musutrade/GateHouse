## Why this fails

The preliminary machine result records that evidence finalization has not completed. The preliminary state is intentionally published before final closed-set validation.

## Diagnose and repair

Inspect the affected artifact binding and invocation identity. Correct the producer or path and generate fresh evidence; do not edit manifests to make stale or missing evidence pass.

Inspect the original failure detail and the affected step or artifact. Preserve
the failed evidence and rerun the intended operation after correcting its cause;
this explanation does not change retry policy or the final decision.

## Example

If publication was interrupted, retain the preliminary result and inspect the interrupted stage. Let a valid verification produce finalized evidence; do not manually flip the completion flag.
