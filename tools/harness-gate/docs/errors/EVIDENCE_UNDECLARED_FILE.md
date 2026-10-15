## Why this fails

A publishable file exists in the invocation directory without a matching declaration. The evidence set must account for its publishable members.

## Diagnose and repair

Inspect the affected artifact binding and invocation identity. Correct the producer or path and generate fresh evidence; do not edit manifests to make stale or missing evidence pass.

Inspect the original failure detail and the affected step or artifact. Preserve
the failed evidence and rerun the intended operation after correcting its cause;
this explanation does not change retry policy or the final decision.

## Example

Configure an intended artifact through the supported declaration path, or keep unrelated generated output outside the invocation evidence directory. Regenerate the set; do not blindly add every stray file to a manifest.
