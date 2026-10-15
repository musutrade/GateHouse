## Why this fails

Invocation metadata could not be deserialized into its required record. This differs from well-formed metadata whose invocation identity does not match.

## Diagnose and repair

Inspect the affected artifact binding and invocation identity. Correct the producer or path and generate fresh evidence; do not edit manifests to make stale or missing evidence pass.

Inspect the original failure detail and the affected step or artifact. Preserve
the failed evidence and rerun the intended operation after correcting its cause;
this explanation does not change retry policy or the final decision.

## Example

If invocation.json was truncated during an interrupted write, repair the producing operation and regenerate metadata with the invocation. Do not hand-edit identities or fill missing fields with guessed values.
