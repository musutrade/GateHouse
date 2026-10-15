## Why this fails

The step was cancelled before normal successful completion. Cancellation can leave cleanup and partial output to inspect.

## Diagnose and repair

Inspect the step log, command, dependencies and execution environment. Correct the first failure before rerunning; a skipped or cancelled step is not a pass.

Inspect the original failure detail and the affected step or artifact. Preserve
the failed evidence and rerun the intended operation after correcting its cause;
this explanation does not change retry policy or the final decision.

## Example

If a run was interrupted, check the recorded cleanup outcome and resource ownership before rerunning. Do not convert the partial step report into a pass or assume all child processes are gone solely because the CLI returned.
