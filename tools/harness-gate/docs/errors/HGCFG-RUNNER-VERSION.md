## Why this fails

The selected runner declares an unsupported contract version. The workflow version and the runner contract version are separate fields.

## Diagnose and repair

use the supported runner contract version

Use the original diagnostic path, location and related entries to identify the
configuration to change. Run `harness-gate config check` after the repair; a
corrected field alone does not prove the whole workflow is valid.

## Example

If a copied runner block was produced for another contract revision, compare its fields with the installed runner documentation. Update the block coherently instead of changing the top-level workflow version.
