# Normalized evidence and capability contracts v1

Current runtime ownership and Python freeze/retirement rules are defined in the
[Python retention policy](python-retention.md). The released Rust core owns final
generic decisions; Python interfaces below remain adapter/reference tooling.


GH-112 implements OpenSpec tasks 2.1–2.5 and 3.1–3.4 as standalone development
contracts. The [schema](../../tools/quality/schema/harness-evidence.schema.json)
and [validator](../../tools/quality/harness_evidence.py) define
`harness-evidence/v1` alongside the unchanged Rust
[`quality-evidence.schema.json`](../../tools/quality/schema/quality-evidence.schema.json).
The [migration inventory](migration-compatibility.md) and
[ADR-0039](../adr/0039-required-risk-and-traceability-gates.md) still govern the
required Rust path. Standalone collector, policy and ratchet contracts now exist;
the [Rust reference adapter](rust-reference-adapter.md) adds retained-evidence
shadow comparison. Required-gate rollout, baseline acceptance and the architecture
ADR remain later tasks.

## Envelope and validation boundary

An envelope contains an evidence ID, project/component, collector name/version,
measurement series, full [project-model subject](project-model.md), typed metrics,
capabilities, commit/base-commit/target/run context, raw artifacts, source path and
SHA-256, and measurement status. A batch must be nonempty. Evidence IDs and
subject/series pairs are unique within the complete submitted batch; metric,
capability and artifact IDs cannot repeat within a record.

`validate_evidence(records, project=..., source_root=..., artifact_root=...,
expected=...)` requires caller-owned inputs. `expected` contains all four context
fields, including an explicit base commit. No field defaults to the current
checkout, another run or an inferred baseline. The validated project supplies
the exact subject, ownership, source digest and target. The collector must equal
the collector encoded by the series. Unsupported fields, states, malformed
digests, duplicate JSON keys, non-finite numbers and mismatches raise
`MeasurementError`; the CLI exits 1 and writes `status: measurement_error`.

Every artifact records a canonical relative POSIX path, byte count, lowercase
SHA-256 of its original bytes, media type, context and source identity. Source
paths resolve under `source_root`, artifact paths under `artifact_root`; absolute,
parent-traversal, noncanonical and escaping symlink paths fail. Files must exist
and match their declared bytes/digests. Artifact context and source must match
the requested evidence exactly. Capability and metric references must resolve
to declared artifacts; unused and duplicate artifact declarations fail.

Raw bytes are retained and hashed without reserialization. The generic validator
does not reinterpret tool-native payloads or certify that an untrusted collector
measured honestly: adapter-specific parsers must establish that correspondence.
Freshness means matching the caller's context and evaluated source bytes, not
file modification time. Callers must retain an immutable input snapshot during
validation and subsequent consumption; this standalone checker is not a sandbox.

`canonical_serialize` performs full validation, then emits UTF-8 JSON with sorted
object keys, compact separators, unescaped Unicode, no trailing newline, and
unchanged array order. Floats and surrogate/control characters are forbidden.
This is the v1 serialization contract, not a claim of RFC 8785 support. A CLI
success records its SHA-256 and means structurally/integrity-valid evidence;
it does not mean the capabilities were measured successfully or release passed.

## Typed metric namespace

The v1 registry in `METRIC_TYPES` binds each supported generic name to one form.
Unknown names/types fail closed; extensions require a reviewed contract update.

| Form | Example | Rules |
| --- | --- | --- |
| ratio | `{"type":"ratio","covered":4,"total":5}` | Integer raw counts; `0 <= covered <= total`, `total > 0`; derive an exact fraction, never store an independently rounded percentage. Empty denominators need an explicit capability state. |
| count | `{"type":"count","value":3}` | Nonnegative integer, never a JSON boolean. |
| boolean | `{"type":"boolean","value":true}` | JSON boolean, never numeric 0/1. |
| duration | `{"type":"duration","value":1000,"unit":"nanoseconds"}` | Nonnegative integer with explicit fixed unit. |
| size | `{"type":"size","value":128,"unit":"bytes"}` | Nonnegative integer with explicit fixed unit. |
| rational | `{"type":"rational","numerator":433,"denominator":108}` | Exact nonnegative integer fraction with positive denominator. An alternative series type for `risk.crap`; never silently mixed with decimal CRAP. |
| decimal | `{"type":"decimal","value":"3.072"}` | Exact nonnegative decimal string; no exponent, leading zeroes or fractional trailing zeroes. Used for `risk.crap`. |

Coverage and mutation score use ratios; complexity, mutation counts, security
findings, breaking changes and accessibility violations use counts. Contract
validity/drift/compatibility and performance regression use booleans;
`performance.duration` and `bundle.size` use duration and size. Shared keys do
not establish shared measurement algorithms or permit cross-series comparison.

## Capability requirements

Each series lists a sorted, unique metric contract. Every record must declare
exactly those capabilities, including a reason and raw provenance references.
Only `supported` has a typed value; it must have one. `unsupported`,
`not_configured`, `not_collected`, `measurement_error`, and `not_applicable`
must have no value, including fabricated zero or one. Status is derived:
`measurement_error` if any capability errored, otherwise `measured` if any is
supported, otherwise `unavailable`. A supported metric with missing evidence is
an invalid envelope, even when requested as informational.

The separate [requirement schema](../../tools/quality/schema/capability-requirements.schema.json)
defines component/metric requirements with `mode` (`required` or `informational`)
and explicit `on_unavailable` (`blocked` or `measurement_error`).
`evaluate_requirements` first fully validates the batch. Supported metrics return
`available`, informational metrics retain their exact state, and required
non-supported metrics return the selected blocking outcome. Measurement errors
always remain errors. Missing component evidence or missing requested capability
returns `measurement_error`. Results retain the original state and evidence ID.
These are availability decisions only; numeric thresholds and release aggregation
belong to later tasks. Requirements apply to each submitted record in a component;
they do not assert that all project subjects have been collected.

## Measurement series

`measurement-series/v1:<sha256>` hashes the canonical series object excluding
its `id`. Identity includes series name, collector/tool/rule/runtime name and
version, target, source-identity rule name/version, normalization name/version,
and the sorted metric name/type contracts. Adapters must version those contracts
whenever measurement semantics change. Concrete source bytes live in subject
identity, not series identity, so a source change alone need not change series.

`require_compatible_series(base, head)` validates both identities and requires
exact equality. Missing base, stale IDs, tool upgrades and rule/identity/runtime/
normalization/target changes fail closed. There is no migration override or
automatic favorable baseline inheritance in v1. This helper is a compatibility
precondition, not a base/head evidence validator or numeric comparison engine.

## Synthetic validation

The [fixtures](../../tools/quality/fixtures/harness-evidence/README.md) retain four
ecosystems' illustrative native artifacts, all value/state forms, a shared
capability requirement policy and declarative negative mutations. They need no
language toolchains and make no real coverage or adapter-equivalence claim.

```bash
python3 tools/quality/harness_evidence.py \
  tools/quality/fixtures/harness-evidence/polyglot.json \
  --project tools/quality/fixtures/project-model/base.json \
  --source-root tools/quality/fixtures/project-model/sources \
  --artifact-root tools/quality/fixtures/harness-evidence \
  --expected tools/quality/fixtures/harness-evidence/expected.json \
  --output target/quality/harness-evidence-validation.json
```

See [GH-112 validation](gh-112-validation.md) for actual commands and retained
machine evidence. Required CI and full architecture acceptance are separate.
