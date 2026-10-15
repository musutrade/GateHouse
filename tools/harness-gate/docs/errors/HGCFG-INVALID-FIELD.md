## Why this fails

A field has the right general configuration shape but violates a more specific validation constraint. The diagnostic path narrows the investigation.

## Diagnose and repair

update the field to satisfy the documented configuration constraints

Use the original diagnostic path, location and related entries to identify the
configuration to change. Run `harness-gate config check` after the repair; a
corrected field alone does not prove the whole workflow is valid.

## Example

If the diagnostic points to a timeout field, inspect that field's documented range rather than editing an unrelated timeout. Recheck the whole configuration after the local correction because more than one field may be invalid.
