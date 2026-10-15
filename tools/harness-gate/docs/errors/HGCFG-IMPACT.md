## Why this fails

The declared component impact graph is incomplete or violates its versioned graph constraints. Correct incremental scope decisions require a valid graph.

## Diagnose and repair

Review the complete version-1 component graph. Declare an empty dependency list for components without dependencies.

Use the original diagnostic path, location and related entries to identify the
configuration to change. Run `harness-gate config check` after the repair; a
corrected field alone does not prove the whole workflow is valid.

## Example

A component with no dependencies still needs the documented empty dependency list. Review all component declarations and references together; do not silently drop an edge that represents a real dependency.
