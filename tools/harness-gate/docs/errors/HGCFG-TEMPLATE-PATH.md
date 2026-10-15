## Why this fails

A report template or its root cannot be resolved under the required input boundary. Missing files, escaping links and overlap with report output need distinct repairs.

## Diagnose and repair

Inspect the diagnostic path and related locations. Keep readable templates within their configured root and separate from report output; correct missing files or escaping symlinks.

Use the original diagnostic path, location and related entries to identify the
configuration to change. Run `harness-gate config check` after the repair; a
corrected field alone does not prove the whole workflow is valid.

## Example

If a template symlink resolves outside the configured template root, place a trusted template inside the root and update the reference. Keep generated reports separate so a previous report cannot become the next run's template input.
