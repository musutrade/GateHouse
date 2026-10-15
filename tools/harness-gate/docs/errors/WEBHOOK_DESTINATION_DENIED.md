## Why this fails

A webhook URL or resolved address violates the allowed destination policy. A syntactically valid URL is not sufficient authorization to send a report.

## Diagnose and repair

Check the configured destination and egress policy. Use an approved endpoint; do not disable address or redirect checks.

Inspect the original failure detail and the affected step or artifact. Preserve
the failed evidence and rerun the intended operation after correcting its cause;
this explanation does not change retry policy or the final decision.

## Example

If a hostname resolves to a disallowed address, choose the intended approved endpoint and inspect DNS configuration. Do not disable address checks or encode the same destination differently to evade validation.
