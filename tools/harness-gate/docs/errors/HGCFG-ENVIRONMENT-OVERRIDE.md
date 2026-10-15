## Why this fails

A supported environment override has a value that cannot be converted to its required type. A valid configuration file can therefore fail only in a particular shell or CI job.

## Diagnose and repair

Set the override named by the diagnostic path to the required type, or unset it. Do not print secret environment values.

Use the original diagnostic path, location and related entries to identify the
configuration to change. Run `harness-gate config check` after the repair; a
corrected field alone does not prove the whole workflow is valid.

## Example

If a numeric override contains nonnumeric text, correct the override or unset it to use the configured value. Compare the named variable's presence and type across environments without printing secret values.
