## Why this fails

The command completed unsuccessfully without a more specific failure code taking precedence. The actual exit status and step log identify the failing behavior.

## Diagnose and repair

Inspect the step log, command, dependencies and execution environment. Correct the first failure before rerunning; a skipped or cancelled step is not a pass.

Inspect the original failure detail and the affected step or artifact. Preserve
the failed evidence and rerun the intended operation after correcting its cause;
this explanation does not change retry policy or the final decision.

## Example

If a test command reports an assertion failure, fix the behavior or the valid test expectation and rerun. Do not append a shell success fallback that discards the command failure.
