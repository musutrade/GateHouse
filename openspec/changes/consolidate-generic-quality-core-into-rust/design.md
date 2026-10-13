# Design: Rust-authoritative Generic Quality Core

## Current implementation and closure

Tasks 1–6 are accepted through merged PR #158 and successful required CI. The
released Rust binary now owns generic evaluation and project reports. GH-152
applies the [final Python boundary and retirement policy](../../../docs/quality/python-retention.md):
all eight inventoried C modules are frozen non-authoritative references, with
explicit reference-only Python decision CLIs and checked source hashes. A/B
Python tooling remains intentional; D artifacts and semantics remain frozen
until reviewed replacement and retirement. Legacy measurement validation and
collector transport retain their documented mixed roles. No ecosystem support
claim expands. [Final closure evidence](../../../docs/quality/gh-152/README.md)
tracks task 7.3's remaining required-CI/controller acceptance.

The sections below retain the original migration design and stage-specific
records; candidate-only and pending-transfer descriptions describe those stages.

## Context

Harness-Gate has completed the language-agnostic architecture migration and a bounded real TypeScript/Angular second-ecosystem acceptance. The external protocol boundary is now credible: collectors can produce normalized evidence without owning delivery decisions.

However, several generic semantics currently live in Python under `tools/quality/`: `harness_evidence.py`, `project_model.py`, `policy_engine.py`, `policy_ratchet.py`, `cross_component.py`, and `project_report.py`. These modules define behavior that is product semantics rather than merely repository-local CI glue.

The consolidation must preserve behavior before improving implementation. The Python implementation is therefore treated as a compatibility oracle during migration, not as technical debt to delete first.

## Product-boundary decision

Every production-like Python quality module is assigned one category:

- **A — CI/dev tooling:** orchestration, hosted-CI glue, documentation checks, benchmarks, local reports. Remains Python unless there is a separate operational reason to change it.
- **B — Ecosystem adapter:** tool parsing, source-map handling, native-output normalization, fixture collection. May remain Python or any other language behind the collector protocol.
- **C — Generic product semantics:** evidence/project/policy/ratchet/relationship/report rules that decide or define generic outcomes. Migrates into Rust.
- **D — Migration/reference tooling:** equivalence replays, certification/advisory helpers and one-time migration utilities. Retained while useful, then frozen or retired.

The architectural rule is:

> Collectors may be written in any language. Harness-Gate's authoritative generic decision semantics live in the released Rust core.

## Rust module boundaries

The Rust implementation should mirror domain boundaries, not Python filenames. A likely decomposition is:

```text
harness-gate core
├── quality::evidence
│   ├── schema/model validation
│   ├── capability/value types
│   ├── artifact/source integrity
│   └── series compatibility
├── quality::project
│   ├── project/component/subject model
│   ├── source boundaries
│   └── relationships
├── quality::policy
│   ├── typed comparisons
│   ├── rule validation
│   ├── subject selection
│   ├── gate results
│   └── aggregate requiredness
├── quality::ratchet
│   ├── baseline compatibility
│   ├── lineage
│   ├── debt classification
│   └── exception review
├── quality::contracts
│   └── cross-component relationship validation
└── quality::report
    ├── project/component indexes
    ├── local/cross-component aggregates
    └── machine report serialization
```

Exact Rust paths may differ after implementation review, but generic semantics must not be split by ecosystem language.

## Contract strategy

The migration is contract-first. Existing accepted JSON schemas and payloads are the external compatibility surface. Rust must deserialize, validate and serialize those contracts without narrowing accepted behavior or silently widening invalid input.

The initial migration should consume the existing schema fixtures and golden records. Schema changes are not bundled into implementation convenience refactors. If Rust exposes a real ambiguity or an unrepresentable invariant, stop that subtask and propose a separate reviewed spec delta with Python/Rust/TypeScript evidence.

## Differential migration

Each C-class capability moves in stages:

1. Freeze representative positive and negative fixtures from both real ecosystems.
2. Implement a Rust candidate behind a non-authoritative CLI/library entry point.
3. Run Python reference and Rust candidate over byte-identical inputs.
4. Compare canonicalized machine outputs field-by-field for the defined compatibility surface.
5. Investigate every mismatch; no allowlist may convert an unexplained semantic mismatch into success.
6. Only after the slice reaches zero unexplained mismatches may downstream generic semantics depend on the Rust implementation.

Do not recollect expensive native coverage/build evidence merely to test the Rust core. Reuse retained normalized/raw artifacts wherever semantics permit.

## Equivalence surface

Equivalence includes more than pass/fail. At minimum compare:

- accepted/rejected input and measurement-error reason class;
- capability state mapping;
- typed numeric/ratio/boolean comparison outcomes;
- selected subjects and relationships;
- gate state and requiredness;
- blockers and aggregate state;
- base/head series compatibility;
- lineage/move/rename treatment;
- debt state and trend;
- exception-review state;
- evidence/artifact links;
- cross-component contract outcome;
- project/component/local/cross-component aggregates;
- report indexes and stable identifiers where contractually defined.

Human wording may be normalized where it is explicitly non-contractual; machine semantics may not.

## Authority model during migration

GH-151's [authority-transfer review](../../../docs/quality/gh-151/README.md)
records the product CLI, opt-in workflow, differential evidence, rollback,
versioned production measurements and completed hosted acceptance. The original
authority requirements below governed that transfer.

The existing required Rust CI remains authoritative. Python generic semantics act as the reference for the new generic core until acceptance. Rust candidate output is shadow evidence.

Authority transfer occurs only after:

- Rust corpus differential acceptance;
- TypeScript/Angular corpus differential acceptance;
- complete negative matrix acceptance;
- hosted CI evidence;
- explicit documentation of rollback;
- proof that current required check identity and semantics are unchanged.

After transfer, the Rust core is authoritative for generic semantics. Python C-class modules are either removed, converted into thin compatibility wrappers invoking Rust, or frozen as test/reference implementations that cannot approve release decisions.

## Adapter boundary

No ecosystem parser is pulled into the Rust core solely because the core is Rust. TypeScript/Angular collection remains external. Future Python/Java/other adapters can do the same.

The collector protocol remains the process boundary:

```text
native tools -> adapter/collector -> normalized evidence -> Rust core -> gate/project result
```

This preserves language independence without creating a multi-language decision core.

## CLI and library exposure

Prefer Rust library APIs for evidence/project/policy evaluation, with a narrow CLI surface for differential replay and future external integration. The CLI must consume explicit project/policy/evidence/base/selection/artifact/source context and must not infer trusted context from collector-controlled data.

Do not expose a second loosely specified JSON API during migration. Reuse the existing versioned contracts.

## Failure and rollback

The migration is fail-closed:

- parser/validation disagreements block migration;
- missing base context cannot become pass;
- unsupported/not-collected/measurement-error states remain distinct;
- series incompatibility blocks incremental comparison;
- relationship/provenance failures remain blockers;
- invalid exception metadata remains a measurement/configuration error.

Rollback before authority transfer is trivial: disable the Rust shadow path. After authority transfer, rollback may restore the previous release while retaining all differential evidence and Python reference fixtures. Do not delete accepted evidence or baselines as part of rollback.

## Testing strategy

Testing is layered:

- Rust unit tests for typed values, validators, selection, aggregation and ratchet rules;
- schema/golden fixtures shared with Python;
- differential tests against retained Rust evidence;
- differential tests against retained TypeScript/Angular evidence;
- negative mutation/tamper/series/identity/exception/contract cases;
- hosted CI shadow job before transfer;
- full existing Rust regression suite and Python quality suite throughout migration.

## Python dispositions after migration

Expected disposition, subject to inventory evidence:

- `ci_quality.py`, `coverage.py`, `critical_paths*.py`, `contracts.py`, `benchmarks.py`, `docs_consistency.py`: remain A-class Python tooling.
- TypeScript/Angular collectors and native contract collectors: remain B-class adapters/tooling.
- `harness_evidence.py`, `project_model.py`, `policy_engine.py`, `policy_ratchet.py`, `cross_component.py`, `project_report.py`: C-class semantics migrate to Rust; afterward remove/freeze/wrap according to dependency inventory.
- `rust_equivalence.py`, `typescript_advisory.py` and migration-specific replayers: D-class; retain through acceptance, then document freeze/retirement policy.

The [GH-152 final inventory](../../../docs/quality/gh-152/python-boundary.json),
not this historical provisional list, now governs dispositions.

## Task 1 freeze record

GH-146 records the [complete Python boundary](../../../docs/quality/gh-146/python-boundary.md)
and [shared retained corpus](../../../tools/quality/fixtures/generic-core/README.md).
The inventory supersedes the provisional filename dispositions above, including
the mixed generic-validation responsibilities in `collector_runner.py` and
`quality_evidence.py`. No runtime authority moves in this slice.

## Task 2 implementation record

GH-147 adds the candidate `harness-gate-quality-core` workspace library under
`tools/harness-gate/quality-core`. It validates evidence, typed measurements,
capability availability, series compatibility, project ownership and structural
relationships. The CLI does not depend on it. Policy decisions, relationship
evidence evaluation, reports and authority transfer remain later tasks.

The library embeds unchanged accepted schemas, checked against the Python copies
by a Rust test. Canonical identities retain sorted UTF-8 JSON, NFC paths and exact
integers. Typed decoding goes through JSON text to preserve arbitrary-precision
integers, including powers of ten, without an intermediate float conversion.
The Python oracle is invoked only by tests against retained bytes; no collector
is rerun. Schema error wording may report only the first failure, while rejection
classes and domain-specific reasons match the reference.

Both workspace packages run in the required Cargo test command. Existing coverage
and risk collection explicitly select the CLI package to retain the accepted
measurement boundary and baseline. No gate threshold, required-check dependency,
production hotspot inventory or release authority changes. See the
[GH-147 validation record](../../../docs/quality/gh-147/README.md) and
[ADR-0040](../../../docs/adr/0040-language-agnostic-evidence-policy.md).

## Task 4 implementation record

GH-149 replaces the task 3 provenance callback with generic Rust contract
validation. The policy engine validates evidence and trusted source/artifact
context first, then checks provider/consumer bindings before typed comparison.
The same relationship and subject selectors serve policy validation and evidence
provenance validation. Missing provenance remains a measurement error.

Rust project reporting retains the policy result and lossless ordered gate table,
then builds participant and other indexes and reuses policy aggregation for
component/local/cross-component views. Shared participant references do not
increase project blocker counts. Reports retain `mode: shadow`; the CLI has no
candidate dependency. See [GH-149 validation](../../../docs/quality/gh-149/README.md).
Tasks 5–7 and full proposal acceptance remain outstanding.

## Task 5 replay and acceptance record

GH-150 adds a non-authoritative Rust library replay/comparator and a Cargo example
outside the released CLI. The retained-corpus driver verifies hashes, reproduces
the frozen Python oracle, and supplies explicit head/base project, records,
expected context, source/artifact roots, policy, selection and clock. It reuses
retained bytes without native collection. The comparator retains all field-level
mismatches at JSON Pointer paths; absent/null, exact numbers and array order stay
significant. Previously accepted OS missing-file wording variations are reported
separately with both strings and the same logical path/failure category.

The required Rust suite includes complete frozen replay and consolidated negative
assertions alongside the existing reference boundary comparisons. See
[GH-150 acceptance evidence](../../../docs/quality/gh-150/README.md). No required
workflow or release authority changes; tasks 6–7 remain outstanding.

## Task 6 hosted acceptance and authority transfer

GH-151 accepts reviewed candidate `0fc96d7644d623f31e31fe1a512cb68db0b2003b`
after hosted shadow and all existing required CI passed, with all 33 retained
cases, the negative matrix and zero unresolved mismatches. The
[explicit decision and raw evidence](../../../docs/quality/gh-151/README.md)
retain the candidate, hosted merge checkout, comparisons and rollback. A distinct
transfer commit enables the released `quality evaluate` Rust entry point; its
identity and final hosted validation are recorded in PR #158.

Collectors retain protocol v1 and only measure. Generic evaluation and report
construction use the linked Rust core without a Python runtime. The legacy
report mode remains wire-compatible. The required aggregate and its dependencies
are unchanged; shadow output cannot replace a required check. The operator's
versioned measurement extensions include linked core production outside `src/`
with identical analyzer/thresholds on authentic base/head series. Task 7 remains
outside this issue, and this record does not accept the full proposal.
