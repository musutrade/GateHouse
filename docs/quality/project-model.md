# Generic project model v1 (GH-111)

Current runtime ownership and Python freeze/retirement rules are defined in the
[Python retention policy](python-retention.md). The released Rust core owns final
generic decisions; Python interfaces below remain adapter/reference tooling.


This implements OpenSpec tasks 1.1–1.4 as a standalone, development-only model.
The [schema](../../tools/quality/schema/project-model.schema.json) and
[validator](../../tools/quality/project_model.py) do not alter project-local
configuration, the shipped Rust CLI, measurement series or required gates.
The [migration inventory](migration-compatibility.md) freezes those boundaries.

| Entity | Contract |
| --- | --- |
| Project | `harness-project/v1`, stable project ID, components, subjects, relationships, descriptive metadata |
| Component | Project-unique ID, canonical repository-relative root, metadata, targets and source boundaries |
| Target | Component-local ID, explicit nonempty boundary references, descriptive runtime/build metadata |
| SourceBoundary | Component-local ID, path contained by the component, `production`, `test`, `generated` or `contract` role |
| Subject | Canonical ID, versioned kind, component/target/boundary references, source path/digest, qualified discriminator, optional span |
| Relationship | Project-unique ID, versioned relationship kind, explicit producer/consumer components and subject references |

Metadata is a string-valued object. Language/framework names are metadata;
unknown ecosystems use exactly the same validation. Relationships are directed,
may form cycles, and do not prescribe execution order or evaluate contract gates.
Referenced subjects must belong to one of the two participants. Missing entities,
duplicate IDs/edges, self relationships, conflicting ownership or digests for
the same source path, and source paths outside their declared boundary fail.

## Subject identity

The closed kind registry is `project/v1`, `component/v1`, `boundary/v1`, `file/v1`,
`function/v1`, `method/v1`, `class/v1`, `route/v1`, `endpoint/v1`, `contract/v1`,
`dependency/v1`, and `critical_path/v1`. Unknown kinds or versions fail; adding
one requires a reviewed registry/schema change. No language registry is needed.

An ID is `subject-identity/v1:` followed by SHA-256 of canonical JSON containing
`identity_version`, `project`, `component`, `target`, `boundary`, `kind`, `path`,
`discriminator`, optional `span`, and `source_sha256`. Serialization uses sorted
keys, no whitespace, UTF-8, and unescaped Unicode (`ensure_ascii=False`).
Discriminators and paths must be NFC; paths must already be canonical POSIX
repository-relative paths without traversal, backslashes, drive prefixes or
control characters. Spans use one-based positions with an inclusive start and
exclusive end. Adapters must normalize their coordinate conventions first.

Adapters provide a qualified symbol or equivalent stable discriminator (for
example a qualified method signature for overloads, an endpoint method/path,
or a contract name), never an authoritative short name alone. Distinct components,
paths, kinds, discriminators and spans remain distinct. Identical records and
conflicting bytes at one source location fail closed. Adapters unable to provide
an unambiguous discriminator must reject collection; reduced identity capability
negotiation is outside this implementation.

`source_sha256` is required, lowercase, and exactly 64 hex characters. File-backed
subjects use a digest of the source file bytes; synthetic/non-file subjects must
reference a digestible declaration (such as a manifest or contract). This model
checks identity integrity and graph consistency, not filesystem bytes or artifact
freshness. Future evidence validation must bind those assertions to retained
artifacts. Both fixtures' source bytes are independently checked by tests.
Metadata does not affect identity. Source, span, target or boundary changes do;
the model makes no automatic historical comparison for them.

## Explicit lineage, not baseline acceptance

The [mapping schema](../../tools/quality/schema/subject-mappings.schema.json)
uses `subject-mappings/v1`, project ID, and mappings with `kind`, `from`, `to`,
and a nonempty `reason`. Base and head projects must validate independently.

- Modify: one retired base identity to one new head identity, preserving component,
  target, boundary, kind, path and discriminator; source digest or span may change.
- Rename: one retired base identity to one new head identity, same component/path
  and subject kind, different qualified discriminator.
- Move: one retired base identity to one new head identity, different component
  or path, same discriminator and subject kind.
- Split: one retired base identity to two or more distinct new head identities,
  all with the same subject kind. Destinations are explicitly enumerated.

All mappings preserve the target. Unknown IDs, repeated sources/destinations, an old identity still present at head,
destinations already present at base, cross-project mappings, invalid cardinality,
and unsupported operations fail. Combined move-and-rename and merge are not v1
operations; no heuristic attempts to infer their lineage.

`baseline_identity` accepts only exact unchanged identity or explicit validated
lineage. A new or changed identity without mapping raises `ModelError`; it cannot
inherit favorable debt or baseline state by short name, path or source similarity.
Resolving lineage does **not** copy metric values or approve favorable inheritance
for a split. The [shadow ratchet](policy-engine.md) independently checks base
provenance and measurement-series compatibility, and treats split children as new
subjects without a debt allowance. It does not accept a production baseline.

The [fixtures](../../tools/quality/fixtures/project-model/README.md) exercise the
model without installing Angular, Rust, Python or Java measurement toolchains.
The normative requirements remain in the
[OpenSpec](../../openspec/changes/language-agnostic-evidence-policy-architecture/specs/language-agnostic-project-model/spec.md).

## Opt-in configuration and contract evaluation

GH-117 adds an explicit JSON manifest and relationship-scoped shadow contract
rules on top of this graph. See [project reporting](project-reporting.md) for
placement, unchanged `flow.toml` defaults, machine indexes and executable single
Rust and synthetic polyglot examples. Graph validation itself still does not
run collectors or decide gate outcomes.
