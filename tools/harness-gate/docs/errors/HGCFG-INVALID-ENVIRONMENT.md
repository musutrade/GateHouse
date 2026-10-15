## Why this fails

An environment-variable identifier does not satisfy the accepted naming rules. This concerns the name, not disclosure of its value.

## Diagnose and repair

use an uppercase environment-variable name containing only letters, digits, and underscores

Use the original diagnostic path, location and related entries to identify the
configuration to change. Run `harness-gate config check` after the repair; a
corrected field alone does not prove the whole workflow is valid.

## Example

Replace a name containing a dash, such as BUILD-MODE, with a supported name such as BUILD_MODE and update its consumers. Never paste secret values into a diagnostic report to demonstrate the fix.
