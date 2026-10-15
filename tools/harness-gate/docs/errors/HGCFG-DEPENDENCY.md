## Why this fails

A declared prerequisite violates the step dependency rules. The diagnostic path and related entries identify the declaration to inspect.

## Diagnose and repair

remove the invalid dependency or add an explicit acyclic prerequisite

Use the original diagnostic path, location and related entries to identify the
configuration to change. Run `harness-gate config check` after the repair; a
corrected field alone does not prove the whole workflow is valid.

## Example

A dependency on the step itself cannot be fulfilled. Correct the referenced prerequisite to the intended distinct step, preserving the ordering needed by the commands and shared resources.
