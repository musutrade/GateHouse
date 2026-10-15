## Why this fails

The workflow cannot be deserialized as the supported configuration. This may
be invalid TOML syntax, an unsupported field or an incompatible value type.
The location and inline diagnostic identify where to begin inspection.

## Diagnose and repair

Inspect the original configuration at the reported line and column. Compare
field names and types with the configuration reference. Keep secret values out
of shared logs and rerun `harness-gate config check` after editing.

## Example

A TOML string must be quoted. This fragment is invalid:

```toml
[project]
name = my-project
```

The corrected fragment is:

```toml
[project]
name = "my-project"
```

These are fragments, not complete workflow configurations. Other required
fields and security configuration files must still pass validation.
