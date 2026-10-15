## Why this fails

Output capture recorded a reader error under this failure class. Inspect the concrete detail: the class is not a guarantee that every instance has exactly the same timeout cause.

## Diagnose and repair

Check whether child processes still hold stdout or stderr open. Ensure descendants terminate and close inherited pipes before retrying.

Inspect the original failure detail and the affected step or artifact. Preserve
the failed evidence and rerun the intended operation after correcting its cause;
this explanation does not change retry policy or the final decision.

## Example

A descendant retaining an inherited output pipe can prevent completion even after its parent exits. Inspect process cleanup and pipe ownership, fix the producer lifecycle, and retry; do not treat an unfinished capture as complete evidence.
