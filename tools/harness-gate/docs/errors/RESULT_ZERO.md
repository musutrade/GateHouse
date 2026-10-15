## Why this fails

The parser observed zero results while the configured minimum required more than zero. The parser evidence records both the observed count and the minimum.

## Diagnose and repair

Inspect the declared result format, output path and parser settings. Ensure the producing test command completed and emitted the required results.

Inspect the original failure detail and the affected step or artifact. Preserve
the failed evidence and rerun the intended operation after correcting its cause;
this explanation does not change retry policy or the final decision.

## Example

A test filter selecting no tests can yield a successful producer exit but still violate the required count. Correct the selection and rerun; do not lower the minimum or fabricate a test result to hide the empty run.
