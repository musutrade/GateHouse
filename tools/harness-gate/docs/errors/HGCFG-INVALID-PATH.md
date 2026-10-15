## Why this fails

A path violates the configured repository boundary or required path shape. A file existing on disk does not make an escaping path valid.

## Diagnose and repair

use a non-empty path below the repository without a prefix or parent traversal

Use the original diagnostic path, location and related entries to identify the
configuration to change. Run `harness-gate config check` after the repair; a
corrected field alone does not prove the whole workflow is valid.

## Example

A configured path containing parent traversal such as ../shared is not repaired by disabling validation. Place the intended input within the allowed project boundary or select the correct project root through the supported CLI options.
