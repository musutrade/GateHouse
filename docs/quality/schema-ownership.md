# Schema source of truth

This note assigns a single editable source to every JSON Schema used by
Harness-Gate. It implements decision D4 of
[the core/plugin split](../../openspec/changes/split-core-and-plugins/design.md):
schemas are not merged by filename; each role has exactly one owner.

| Role | Owner | Editable root | Contents |
| --- | --- | --- | --- |
| Host contract (request/decision wire format) | core (GateHouse) | [`schema/`](../../schema/README.md) | `flow`, `machine-result`, `artifact-registry`, `artifact-manifest`, `adapter-request`, `quality`, and the collector protocol contract |
| Generic evidence / policy | core (GateHouse) | [`tools/harness-gate/quality-core/schema/`](../../tools/harness-gate/quality-core/schema) | `policy`, `policy-exceptions`, `harness-evidence`, `capability-requirements`, `project-model`, `subject-mappings` |
| Plugin raw output | plugin (GateProbes) | [`tools/quality/schema/`](../../tools/quality/schema) | `rust-collector-*`, `project-config`, `quality-evidence` |

The generic-evidence role is authored once under
`tools/harness-gate/quality-core/schema/`, the decision body that embeds it via
`include_str!`. The Python reference and CI tooling in `tools/quality/` consume
byte-identical copies under `tools/quality/schema/`. Those copies are
**generated**: they must not be edited independently, and any change to a source
is mirrored before review.

## Generated copies

| Generated copy | Editable source |
| --- | --- |
| `tools/quality/schema/capability-requirements.schema.json` | `tools/harness-gate/quality-core/schema/capability-requirements.schema.json` |
| `tools/quality/schema/harness-evidence.schema.json` | `tools/harness-gate/quality-core/schema/harness-evidence.schema.json` |
| `tools/quality/schema/policy-exceptions.schema.json` | `tools/harness-gate/quality-core/schema/policy-exceptions.schema.json` |
| `tools/quality/schema/policy.schema.json` | `tools/harness-gate/quality-core/schema/policy.schema.json` |
| `tools/quality/schema/project-model.schema.json` | `tools/harness-gate/quality-core/schema/project-model.schema.json` |
| `tools/quality/schema/subject-mappings.schema.json` | `tools/harness-gate/quality-core/schema/subject-mappings.schema.json` |

The machine-readable manifest is
[`schema-ownership.json`](schema-ownership.json). The invariant is enforced by
`tools/quality/tests/test_schema_ownership.py`, which fails if any duplicated
basename has more than one editable source or if a generated copy diverges from
its source.

A release package may ship a generated copy, but the two sides must not edit
their own versions independently.
