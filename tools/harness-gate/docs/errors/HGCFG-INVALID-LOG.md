## Why this fails

A step log is not a valid single .log filename. Log destinations must remain unambiguous within the invocation output area.

## Diagnose and repair

choose a unique single .log filename

Use the original diagnostic path, location and related entries to identify the
configuration to change. Run `harness-gate config check` after the repair; a
corrected field alone does not prove the whole workflow is valid.

## Example

Use a filename such as unit-tests.log instead of a directory traversal or an unrelated extension. Each step must also have a distinct log identity; changing only the extension may not resolve a duplicate-log error.
