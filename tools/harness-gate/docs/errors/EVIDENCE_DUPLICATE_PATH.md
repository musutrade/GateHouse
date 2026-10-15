## Why this fails

One artifact path has conflicting kind declarations or conflicting step bindings. Compatible repeated declarations can be coalesced; the failure concerns conflicting ownership or meaning.

## Diagnose and repair

Inspect the affected artifact binding and invocation identity. Correct the producer or path and generate fresh evidence; do not edit manifests to make stale or missing evidence pass.

Inspect the original failure detail and the affected step or artifact. Preserve
the failed evidence and rerun the intended operation after correcting its cause;
this explanation does not change retry policy or the final decision.

## Example

Give two independently produced artifacts distinct declared paths and bind each to its actual step. Do not delete one binding while retaining ambiguous bytes under the shared path.
