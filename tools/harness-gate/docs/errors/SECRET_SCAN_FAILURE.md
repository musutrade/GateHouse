## Why this fails

The built-in secret scan gate did not pass. Inspect its findings or error detail without exposing credential values.

## Diagnose and repair

Inspect the finding without printing the secret. Remove exposed credentials from tracked inputs and rotate compromised values before rerunning the scan.

Inspect the original failure detail and the affected step or artifact. Preserve
the failed evidence and rerun the intended operation after correcting its cause;
this explanation does not change retry policy or the final decision.

## Example

For a detected tracked credential, remove the exposed value and rotate it through the appropriate system before rerunning scanning. If scanning itself failed, repair that execution problem; no findings from an incomplete scan do not establish a pass.
