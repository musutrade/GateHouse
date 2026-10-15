## Why this fails

The number of independent errors exceeds the bounded diagnostic response. This notice means the displayed list is incomplete, not that omitted errors are ignored.

## Diagnose and repair

Fix the reported errors and run harness-gate config check again to reveal remaining diagnostics.

Use the original diagnostic path, location and related entries to identify the
configuration to change. Run `harness-gate config check` after the repair; a
corrected field alone does not prove the whole workflow is valid.

## Example

Fix the first reported group and run config check again. Additional errors may appear on the next run; retain a failing status until the complete configuration validates.
