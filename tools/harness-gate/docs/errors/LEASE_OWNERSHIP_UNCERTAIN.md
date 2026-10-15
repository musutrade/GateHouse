## Why this fails

Allocation, cleanup or renewal cannot prove the required owner identity. This can include unavailable process identity, an unreadable live holder identity or a failed heartbeat.

## Diagnose and repair

Inspect the recorded owner and runtime identity. Use cleanup --dry-run to inspect stale leases; retain uncertain resources rather than deleting by name.

Inspect the original failure detail and the affected step or artifact. Preserve
the failed evidence and rerun the intended operation after correcting its cause;
this explanation does not change retry policy or the final decision.

## Example

A PID being present does not prove it is the original owner. Inspect cleanup --dry-run and retained owner metadata; retain uncertain resources instead of deleting by a reused name or removing the ownership record to force progress.
