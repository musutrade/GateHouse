## Why this fails

Steps use a shared service without the required ordering. Their operations may interfere even when each command is valid in isolation.

## Diagnose and repair

Add an explicit dependency between the affected steps or define separate service resources.

Use the original diagnostic path, location and related entries to identify the
configuration to change. Run `harness-gate config check` after the repair; a
corrected field alone does not prove the whole workflow is valid.

## Example

If two steps mutate the same shared resource, declare the intended dependency or configure separate service resources. Do not add a sleep as a substitute for an explicit dependency.
