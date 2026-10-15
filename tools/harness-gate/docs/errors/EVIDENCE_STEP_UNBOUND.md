## Why this fails

A reported step lacks the invocation-bound step identity required for publication. A human label alone does not supply this identity.

## Diagnose and repair

Inspect the affected artifact binding and invocation identity. Correct the producer or path and generate fresh evidence; do not edit manifests to make stale or missing evidence pass.

Inspect the original failure detail and the affected step or artifact. Preserve
the failed evidence and rerun the intended operation after correcting its cause;
this explanation does not change retry policy or the final decision.

## Example

Repair the producer that constructs the step result so it receives the correct invocation-bound identity. Regenerate the evidence; do not invent a step ID by editing the finished report.
