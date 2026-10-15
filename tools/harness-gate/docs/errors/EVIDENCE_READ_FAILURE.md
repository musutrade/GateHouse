## Why this fails

Evidence inspection or reading failed. The path may exist but its bytes could not be reliably consumed for validation.

## Diagnose and repair

Inspect the affected artifact binding and invocation identity. Correct the producer or path and generate fresh evidence; do not edit manifests to make stale or missing evidence pass.

Inspect the original failure detail and the affected step or artifact. Preserve
the failed evidence and rerun the intended operation after correcting its cause;
this explanation does not change retry policy or the final decision.

## Example

Repair the specific access or filesystem problem and produce a new verifiable evidence set. A stale checksum or previously read copy does not prove the current unreadable artifact is valid.
