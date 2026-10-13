# Plugin and generated schemas

This directory holds **plugin raw-output** schemas owned by GateProbes
(`rust-collector-*`, `project-config`, `quality-evidence`) and **generated
copies** of the core generic-evidence schemas.

## Generated copies — do not edit

The following files are byte-identical copies generated from the core decision
body at [`tools/harness-gate/quality-core/schema/`](../../harness-gate/quality-core/schema).
Edit the source, then mirror the change; the copies are checked by
`tools/quality/tests/test_schema_ownership.py`.

- `capability-requirements.schema.json`
- `harness-evidence.schema.json`
- `policy-exceptions.schema.json`
- `policy.schema.json`
- `project-model.schema.json`
- `subject-mappings.schema.json`

See [schema ownership](../../../docs/quality/schema-ownership.md) for the full
role assignment.
