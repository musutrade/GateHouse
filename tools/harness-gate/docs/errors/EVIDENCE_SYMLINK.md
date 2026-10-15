## Why this fails

A symbolic link appears where the evidence contract requires an owned regular file. Following it could substitute data outside the intended evidence set.

## Diagnose and repair

Inspect the affected artifact binding and invocation identity. Correct the producer or path and generate fresh evidence; do not edit manifests to make stale or missing evidence pass.

Inspect the original failure detail and the affected step or artifact. Preserve
the failed evidence and rerun the intended operation after correcting its cause;
this explanation does not change retry policy or the final decision.

## Example

Make the intended producer emit a regular file in the invocation directory. Do not follow an arbitrary link and copy its target as if those bytes were already trusted current evidence.
