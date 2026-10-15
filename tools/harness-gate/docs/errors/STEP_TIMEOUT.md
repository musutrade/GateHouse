## Why this fails

A step did not finish within its configured execution deadline. Cancellation
and cleanup may run after that deadline; their outcome must also be inspected.
A timeout is not successful evidence even if a partial report exists.

## Diagnose and repair

Read the step log to find its last completed operation. Check for interactive
prompts, unbounded child processes and unavailable dependencies. Reproduce the
command in its declared working directory and environment. Repair the cause
and rerun the original verification.

## Example

A command waiting for a confirmation prompt cannot finish in unattended CI.
Configure that tool's documented noninteractive mode and test its failure
behavior. Do not suppress its exit status or turn the required step optional.
If an intentionally long operation needs a different budget, change it only
through the project's normal reviewed configuration process, not as an
automatic response to a timeout.
