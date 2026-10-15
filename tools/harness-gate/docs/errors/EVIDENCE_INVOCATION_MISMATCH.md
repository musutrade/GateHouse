## Why this fails

A step or invocation metadata record does not match the invocation being published. Valid JSON can still belong to the wrong run or input identity.

## Diagnose and repair

Inspect the affected artifact binding and invocation identity. Correct the producer or path and generate fresh evidence; do not edit manifests to make stale or missing evidence pass.

Inspect the original failure detail and the affected step or artifact. Preserve
the failed evidence and rerun the intended operation after correcting its cause;
this explanation does not change retry policy or the final decision.

## Example

When a stale report was copied from an earlier run, generate fresh evidence for the current invocation. Do not replace only its invocation identifier or input hash to make the old measurement look current.
