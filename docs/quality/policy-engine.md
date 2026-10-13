# Generic policy engine v1

Python commands below require `--reference-only` and cannot approve releases.
See the [freeze and retirement policy](python-retention.md).

GH-114 implements OpenSpec tasks 5.1–5.5; GH-115 adds tasks 6.1–6.4 to the
standalone **shadow** evaluator.
The existing Rust required gates and `Required Quality Aggregate` remain release
truth under [ADR 0039](../adr/0039-required-risk-and-traceability-gates.md).
[Collectors](collector-protocol.md) return facts; policy owns comparisons,
requiredness, violation severity and aggregation. No required CI wiring changes.

The [closed JSON policy schema](../../tools/quality/schema/policy.schema.json)
requires explicit rule IDs, scope, generic metric name, typed limit, operator,
requiredness, violation state and remediation classes. Unknown fields and metric
names, duplicate rule IDs, invalid selectors, mismatched types and executable
expressions are rejected. Operators are `lt/le/eq/ne/ge/gt`; boolean values allow
only `eq/ne`. Ratios retain numerator/denominator and decimals use canonical
strings. Comparisons use exact rational arithmetic, including large counts,
durations and sizes. Limits use the metric's normalized type and unit.

## Scope and evidence ownership

Every rule evaluates each declared subject within the requested target:

| Selector | Selected subjects |
| --- | --- |
| `project` | All project subjects in the target |
| `component` + `component` | Subjects owned by that component |
| `boundary` + `component` + `boundary` | Subjects in that component's source boundary |
| `changed_subject` | Caller-supplied changed subject IDs |
| `critical_subject` | Caller-supplied critical subject IDs |
| `subject` + `subject` | The resolved canonical subject identity |
| `relationship` + `relationship` | Provider-owned contract subjects on that relationship |

The Rust Core validates the exact fields of each scope kind. `project` accepts
only `kind`; adding `component` or `boundary` is an input error rather than a
filter. Component and boundary scopes require their named nonempty fields;
changed/critical scopes accept only `kind`. Relationship scopes require the
relationship ID domain and subject scopes require a canonical subject ID after
compilation. Configuration documents may use a nonempty subject alias before
the compiler resolves it; resolved policy and independent selection validate
canonical identities. Unknown kinds, extra/missing fields and control characters
are rejected without changing the frozen schema walker or reference corpus.

These are selection scopes; they do not average metrics across subjects or
measurement series. A component or boundary summary can be represented as a
subject by the project model. Changed/critical selections are supplied separately
from collector evidence, validated against the project and target, and required
when that selector is used. Missing selected-subject evidence, ambiguous series,
invalid provenance and corrupt artifacts produce `measurement_error`. An empty
selection produces `not_applicable` and cannot satisfy a required rule. A project
with an unmeasured contract subject consequently cannot pass a project coverage
rule merely because its measured functions pass.

The evaluator validates the entire evidence batch through the existing
[evidence contract](harness-evidence.md) before using values. It never trusts a
collector's numerical pass/fail judgment. Unavailable metrics have no fabricated
numeric value. Capability mapping is explicit:

| Capability | Gate result |
| --- | --- |
| `supported` | Comparison gives `pass` or configured `fail/warning/informational` |
| `unsupported` | `unsupported` |
| `not_applicable` | `not_applicable` |
| `not_configured` | `blocked` |
| `not_collected` | `skipped` |
| `measurement_error` | `measurement_error` |

## Results and aggregation

`GateResult` and `GateState` retain `pass`, `fail`, `warning`, `informational`,
`skipped`, `unsupported`, `not_applicable`, `measurement_error`, `blocked`, and
`cancelled`. The latter represents orchestration cancellation without conflating
it with a threshold failure. The aggregate reads requiredness from policy, never
from child records. Required states other than `pass` and `informational` block;
`required: false` explicitly makes a rule non-blocking. A required warning still
blocks. Missing required rules block. Duplicate children and unknown policy IDs
are rejected. Informational results retain their records without blocking.

The aggregate preserves every blocking child and original state. Its summary is
`measurement_error` if any such blocker exists, otherwise `fail` if any threshold
failure exists, otherwise `blocked`, or `pass` with no blockers. Thus mixed
failure/error/cancellation results cannot erase one another's causes.

`harness-policy-results/v1` JSON contains `mode: shadow`, `aggregate`, all
`results`, and non-pass `violations`. Each child has `policy` (rule ID), `subject`
(ID), `state`, `reason`, and a `record` containing:

- component, complete subject identity and generic metric;
- typed base/head values (null when unavailable) and commit/base/target/run context;
- complete governing policy, including the typed limit and operator;
- base/head measurement-series IDs and evidence IDs with artifact paths/digests;
- remediation classes from policy for comparisons, or `repair_measurement` for
  unavailable/invalid measurements.

## Compatible baselines, debt and regression

A rule may add `"ratchet": {"deny_regression": true, "allow_legacy_debt": true}`
alongside its absolute limit. Both flags are required when ratchet is present.
Without ratchet, existing absolute evaluation remains supported, including optional
base values for remediation. Caller-owned `base_records` and `base_context` must
provide the base project, source/artifact roots and provenance separately. The base
commit must equal head `base_commit`; project and target must match. Both batches
pass full source/artifact and series validation. Missing/ambiguous base metrics,
unavailable historical values or incompatible series produce `measurement_error`.
New subjects also require a matching series in their component's base evidence;
an absent baseline is never equivalent to an empty, passing project.

History uses the [GH-111 identity model](project-model.md). Exact identities are
`unchanged`. Explicit one-to-one `modify`, `rename` or `move` mappings classify the
head as `modified` and allow comparison only after series validation. `modify`
extends the mapping schema for content/span changes: component, target, boundary,
kind, path and discriminator must remain equal. Every mapping requires exact IDs
and a reason; retired sources and unique destinations prevent ambiguous joins.
There is no symbol/path similarity fallback. An unmapped new identity is `new`,
including a content edit without a reviewed mapping. Split children are also `new`
for policy: their source lineage is recorded, but no historical metric or debt
allowance is copied. All lineage mappings preserve target.

For ordered operators (`lt/le/gt/ge`), regression is movement away from the limit
(lower values are better for maxima, higher for minima). Equality/inequality rules
compare compliance: compliant to noncompliant regresses, the reverse improves,
and changes with the same compliance remain unchanged. All numeric comparisons
retain exact typed arithmetic. Subject classification describes source identity;
metric `trend` independently describes measured change.

| Base → head, maximum 30 | Absolute compliant | Debt | Default ratchet result |
| --- | --- | --- | --- |
| 64 → 64 | false | unchanged | informational |
| 64 → 55 | false | improved | informational |
| 64 → 65 | false | regressed | fail |
| 18 → 27 | true | none | fail (regression) |
| 31 → 30 | true | resolved | pass |
| new → 31 | false | new | fail |

Here the default example uses both ratchet flags above and `on_violation: fail`.
`allow_legacy_debt: false` enforces the absolute limit even on improving legacy
subjects. `deny_regression: false` permits regressions that remain compliant;
worsening debt still violates the absolute rule. Severity and requiredness remain
policy-owned. An informational legacy-debt result permits an incremental aggregate
pass while retaining `absolute_compliant: false`; it never certifies whole-project
compliance. Policies select only their declared scope, including caller-owned
changed/critical subject selections.

Each compared result adds `baseline` (classification, lineage and inheritance)
and `ratchet` (base/head absolute compliance, trend, debt, remaining debt, regression
and legacy allowance). `debt_ledger` retains complete result records for new,
unchanged, improved, regressed and resolved debt, with values, series, policy and
raw evidence links. Resolved entries record progress; unavailable comparisons
remain explicit errors rather than fabricated ledger values. This ledger is a
derived report for the validated base/head pair, not an independently trusted
baseline database or a claim about unselected subjects.

## Exception review metadata

The API accepts a separate `exceptions` array using the
[closed exception schema](../../tools/quality/schema/policy-exceptions.schema.json).
Each entry binds a known policy ID and head subject ID and requires nonblank
`owner`, `issue`, `reason`, `expiry` and `compensating_control`; optional `approver`
is preserved. Expiry is an ISO datetime with timezone, strictly later than the
current UTC clock (the API accepts an injected aware `now` for reproducible tests).
Unknown fields/references, duplicate policy/subject records, missing values,
malformed timestamps and expired entries block with `measurement_error`.

Valid metadata changes `exception_review` to `documented` and attaches the original
metadata to the result. It never changes the quality state. `quality_aggregate`
retains the original gate outcome; `aggregate` additionally blocks invalid exception
metadata. A failed result stays failed with either valid or invalid exceptions.
No exception governance waiver or approval authority is implemented.

## Standalone example and validation

Run from the repository root:

```bash
python3 tools/quality/policy_engine.py --reference-only \
  --policy tools/quality/fixtures/policy/policy.json \
  --evidence tools/quality/fixtures/harness-evidence/polyglot.json \
  --project tools/quality/fixtures/project-model/base.json \
  --source-root tools/quality/fixtures/project-model/sources \
  --artifact-root tools/quality/fixtures/harness-evidence \
  --expected tools/quality/fixtures/harness-evidence/expected.json \
  --output target/quality/policy-example.json
```

Expected exit code: **1**. Frontend coverage passes (4/5); CRAP 3.072 exceeds
3 and produces a structured failure. This synthetic example certifies no real
ecosystem adapter. `--selection` accepts an object with `changed_subject` and/or
`critical_subject` ID arrays. For incremental evaluation, supply all five `--base-evidence`, `--base-project`,
`--base-source-root`, `--base-artifact-root`, and `--base-expected` paths. Optional
`--mappings` supplies `subject-mappings/v1`; `--exceptions` supplies the metadata
array. Partial base arguments fail closed. The CLI exits zero only for aggregate pass; invalid
policy/input emits a machine-readable measurement error and exits one.

[Focused tests](../../tools/quality/tests/test_policy_engine.py) cover scope
ownership, exact comparison boundaries, coverage/CRAP/mutation/breaking counts
and booleans across four synthetic ecosystems, capability states, corrupt and
missing evidence, every aggregate state, base-series compatibility, remediation
serialization and CLI exit behavior. Actual command evidence is recorded in the
[GH-114 validation summary](https://github.com/musutrade/Harness-Gate/blob/d9a932b23dcfea709175285b594ca200450a9971/docs/quality/gh-114/validation-summary.json).

[GH-115 focused tests](../../tools/quality/tests/test_policy_ratchet.py) exercise
four synthetic ecosystems and negative identity/series/exception paths. See the
[GH-115 validation record](gh-115-validation.md) and retained machine evidence.
The [Rust reference adapter](rust-reference-adapter.md) compiles the existing Rust
policy into this engine for shadow comparison. Exact rational CRAP values and
limits compare as integer fractions; decimal and rational series cannot mix.
Required CI rollout remains a later task.
