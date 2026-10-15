## Why this fails

The built-in architecture audit gate did not pass. The associated audit result or error identifies the violation or failed operation.

## Diagnose and repair

Inspect the reported rule and source location. Correct the dependency or boundary violation, then rerun the audit.

Inspect the original failure detail and the affected step or artifact. Preserve
the failed evidence and rerun the intended operation after correcting its cause;
this explanation does not change retry policy or the final decision.

## Example

For a prohibited dependency, restructure the code to respect the reviewed boundary and rerun the audit. Do not remove the enforcing rule or suppress the audit exit status merely to obtain a green verification.
