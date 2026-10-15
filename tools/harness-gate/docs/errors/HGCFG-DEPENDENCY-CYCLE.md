## Why this fails

Step dependencies contain a cycle, so there is no valid order that satisfies every prerequisite.

## Diagnose and repair

remove the invalid dependency or add an explicit acyclic prerequisite

Use the original diagnostic path, location and related entries to identify the
configuration to change. Run `harness-gate config check` after the repair; a
corrected field alone does not prove the whole workflow is valid.

## Example

If step A requires B and B requires A, identify the real data dependency. Remove the incorrect edge or separate shared preparation into a prerequisite. Do not remove a necessary check merely to break the cycle.
