## Why this fails

A step was not executed under the scheduler decision. Its recorded detail describes the reason; skipped is not an interchangeable spelling for passed.

## Diagnose and repair

Inspect the step log, command, dependencies and execution environment. Correct the first failure before rerunning; a skipped or cancelled step is not a pass.

Inspect the original failure detail and the affected step or artifact. Preserve
the failed evidence and rerun the intended operation after correcting its cause;
this explanation does not change retry policy or the final decision.

## Example

After a prerequisite fails, repair it before rerunning the required dependent steps. Do not reuse a previous dependent result or alter the skipped flag to claim current verification coverage.
