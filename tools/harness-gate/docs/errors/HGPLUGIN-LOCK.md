# HGPLUGIN-LOCK

## Why this fails

The plugin lock is not `harness-plugin-lock/v1`, selects no plugins or more than 64, or selects the same plugin name twice.

## Diagnose and repair

Use a v1 lock with 1..64 unique exact pins. Review migration before replacing an existing lock; absence of a lock preserves legacy workflows.

## Example

`harness-gate explain HGPLUGIN-LOCK`
