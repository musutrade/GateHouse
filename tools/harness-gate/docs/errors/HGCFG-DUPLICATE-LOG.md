## Why this fails

Two verification steps claim the same normalized log identity. Their output cannot be safely attributed to separate steps.

## Diagnose and repair

Choose a unique .log filename for each verification step; use the related location to find the other owner.

Use the original diagnostic path, location and related entries to identify the
configuration to change. Run `harness-gate config check` after the repair; a
corrected field alone does not prove the whole workflow is valid.

## Example

Give the conflicting steps distinct filenames such as unit.log and integration.log. Use the related diagnostic location to find the other declaration; changing only a visually different spelling may still collide after normalization.
