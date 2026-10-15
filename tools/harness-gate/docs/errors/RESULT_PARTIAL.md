## Why this fails

The parser observed some results but fewer than the required minimum. This differs from malformed output and from a zero-result run.

## Diagnose and repair

Inspect the declared result format, output path and parser settings. Ensure the producing test command completed and emitted the required results.

Inspect the original failure detail and the affected step or artifact. Preserve
the failed evidence and rerun the intended operation after correcting its cause;
this explanation does not change retry policy or the final decision.

## Example

If only one shard produced a report, collect the intended complete set from the current invocation and rerun as required. Preserve the required minimum; an older shard report is not equivalent to a missing current result.
