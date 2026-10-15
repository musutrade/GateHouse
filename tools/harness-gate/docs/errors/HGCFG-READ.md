## Why this fails

The workflow file cannot be read as bounded UTF-8 input. Missing files, unreadable files, invalid encoding and the configuration size limit can prevent loading.

## Diagnose and repair

Check that the configured file exists, is readable UTF-8 and is below the 4 MiB size limit.

Use the original diagnostic path, location and related entries to identify the
configuration to change. Run `harness-gate config check` after the repair; a
corrected field alone does not prove the whole workflow is valid.

## Example

For an incorrect --config path, select the intended file and confirm permissions. For an oversized generated file, reduce the configuration at its producer; do not increase the limit or truncate arbitrary bytes into invalid TOML.
