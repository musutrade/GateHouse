## Why this fails

Captured stdout, stderr or combined output exceeded the allowed bound. Retained output can be truncated and is not necessarily the complete command transcript.

## Diagnose and repair

Reduce unnecessary command output or correct runaway logging. Inspect the explicit output budget; truncated output cannot establish a successful result.

Inspect the original failure detail and the affected step or artifact. Preserve
the failed evidence and rerun the intended operation after correcting its cause;
this explanation does not change retry policy or the final decision.

## Example

A command emitting an unbounded debug loop should be repaired or configured to produce bounded useful output. Do not ignore truncation or assume a parser saw complete results from the truncated capture.
