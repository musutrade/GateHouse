## Why this fails

The optional quality configuration is present but invalid. Its references, profile participation or policy and collector authority must be consistent.

## Diagnose and repair

Correct quality.toml references, profile participation and policy/collector authority. Preserve required checks and thresholds.

Use the original diagnostic path, location and related entries to identify the
configuration to change. Run `harness-gate config check` after the repair; a
corrected field alone does not prove the whole workflow is valid.

## Example

A quality rule referencing a missing component requires correcting the reference or defining the intended component. Do not remove quality.toml or make required checks optional to turn invalid configuration into apparent success.
