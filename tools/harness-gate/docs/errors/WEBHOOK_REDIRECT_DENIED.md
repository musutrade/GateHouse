## Why this fails

The notification endpoint returned a redirect while redirect following is forbidden. The destination of the redirected request has not been accepted by this delivery operation.

## Diagnose and repair

Check the configured destination and egress policy. Use an approved endpoint; do not disable address or redirect checks.

Inspect the original failure detail and the affected step or artifact. Preserve
the failed evidence and rerun the intended operation after correcting its cause;
this explanation does not change retry policy or the final decision.

## Example

Configure the intended approved endpoint directly rather than relying on a redirect. Do not turn off redirect restrictions to make delivery appear successful; inspect the actual notification failure separately from local report completion.
