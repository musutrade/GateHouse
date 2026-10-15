## Why this fails

A configuration identifier is declared more than once. References cannot unambiguously select the intended declaration.

## Diagnose and repair

keep one declaration or give each declaration a unique identifier

Use the original diagnostic path, location and related entries to identify the
configuration to change. Run `harness-gate config check` after the repair; a
corrected field alone does not prove the whole workflow is valid.

## Example

Two steps with the same id need either one intended declaration or distinct identifiers. If renaming one step, update its references and review dependency ordering; do not rely on one declaration silently overriding the other.
