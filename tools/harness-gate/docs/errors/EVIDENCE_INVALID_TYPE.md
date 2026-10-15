## Why this fails

An evidence path has an unsupported filesystem type or shape. A directory or special file is not interchangeable with a required regular artifact.

## Diagnose and repair

Inspect the affected artifact binding and invocation identity. Correct the producer or path and generate fresh evidence; do not edit manifests to make stale or missing evidence pass.

Inspect the original failure detail and the affected step or artifact. Preserve
the failed evidence and rerun the intended operation after correcting its cause;
this explanation does not change retry policy or the final decision.

## Example

If a path expected to contain invocation metadata is a directory, correct the producer layout and regenerate it. Do not replace a special file with arbitrary placeholder JSON just to satisfy the type check.
