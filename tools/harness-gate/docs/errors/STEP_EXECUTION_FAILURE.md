## Why this fails

Executing an external verification step returned an error to the scheduler. This is distinct from an ordinary completed command with an unsuccessful exit status.

## Diagnose and repair

Inspect the step log, command, dependencies and execution environment. Correct the first failure before rerunning; a skipped or cancelled step is not a pass.

Inspect the original failure detail and the affected step or artifact. Preserve
the failed evidence and rerun the intended operation after correcting its cause;
this explanation does not change retry policy or the final decision.

## Example

If launching the command fails, inspect its executable, arguments, working directory and required environment. Repair the launch condition and rerun the intended step; a command that never launched cannot be treated as a successful test.
