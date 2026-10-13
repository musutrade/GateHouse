# Cross-component contracts and project reports

Python commands below require `--reference-only` and cannot approve releases.
See the [freeze and retirement policy](python-retention.md).

After [GH-151 hosted acceptance](gh-151/README.md), the explicit-context
`harness-gate quality evaluate` entry point produces authoritative generic decisions
and project reports in Rust using the same collector and report contracts.
The historical Python interface documented below remains reference/shadow tooling.

GH-117 implements OpenSpec tasks 8.1–8.3 and 9.1–9.3 in the standalone
**shadow** evaluator. Rust remains the reference adapter. Angular/TypeScript,
Python, Java, OpenAPI comparison and client generation here are **synthetic
fixtures**, not implemented or certified ecosystem adapters. Required Rust CI
and [ADR-0039](../adr/0039-required-risk-and-traceability-gates.md) retain release
authority. CI rollout and architecture acceptance remain separate tasks.

## Configuration placement and compatibility

The opt-in manifest is `.harness-gate/project.json`, with schema
[`harness-project-config/v1`](../../tools/quality/schema/project-config.schema.json):

```json
{
  "schema": "harness-project-config/v1",
  "project_model": ".harness-gate/components.json",
  "policy": ".harness-gate/project-policy.json"
}
```

The referenced documents use the existing `harness-project/v1` project model
and `harness-policy/v1` policy schemas. Components, targets, source boundaries,
subjects and relationships live together in the model document. References are
canonical repository-relative paths resolved against `--root` (default: current
working directory), independently of the manifest's location. Unknown versions,
unknown keys, absolute/traversing paths and symlinks escaping that root fail.
Source and artifact roots and expected commit/base/target/run identity are
explicit evaluation arguments, not collector-controlled manifest defaults.

This is a separate, additive Python CLI. The existing Rust binary continues to
read `.harness-gate/flow.toml` with its existing defaults and source boundaries.
It does not discover this manifest, and the shadow CLI does not read or convert
`flow.toml`. Existing single-component Rust projects need no migration. A missing
shadow manifest is an explicit error; it never manufactures a default project.
Do not paste the conceptual TOML component example from the initial architecture
design into `flow.toml`; the implemented component manifest is JSON.

## Contract subjects and metrics

An existing `contract/v1` subject is owned by its provider component. Each
relationship explicitly names `producer`, `consumer` and subject IDs. A
`relationship` policy scope selects only provider-owned contracts on the
requested target. Multiple consumers use explicit relationship IDs, and separate
evidence series where their measurement semantics differ. Unknown relationships,
missing contracts and unrelated subjects cannot become successful empty scopes.
A `subject` scope selects one exact ID for a component-local gate; existing
project/component/boundary and caller-selected scopes retain their behavior.

| Metric | Type | Example required policy |
| --- | --- | --- |
| `contract.schema_valid` | boolean | equals `true` |
| `contract.breaking_changes` | nonnegative count | equals `0` |
| `contract.client_drift` | boolean | equals `false` |
| `contract.compatible` | boolean | equals `true` |

`contract.client_drift` is the existing registry name for generated-client drift;
no alias or measurement-series rename is introduced. Unsupported and unavailable
capabilities carry no metric value and preserve their blocking policy semantics.
Collectors provide facts; the generic typed comparator evaluates the policy.

Contract evidence adds an optional `contract` object to the normalized envelope;
it is required before any supported relationship-scoped metric is evaluated:

```json
{
  "relationship": "api-client",
  "producer": "api",
  "consumer": "frontend",
  "contract_artifact": "contract",
  "baseline": {
    "artifact": "baseline",
    "commit": "0000000000000000000000000000000000000000",
    "series_id": "measurement-series/v1:<64 lowercase hex characters>"
  },
  "consumer_artifact": "consumer",
  "generated_client": {
    "artifact": "client",
    "contract_sha256": "<64 lowercase hex characters>"
  }
}
```

Artifact IDs resolve to digest-checked retained files and must occur in the
metric's evidence links. The contract artifact digest must equal the subject's
source digest. Breaking-change and compatibility evaluation require the retained
base contract, the caller-pinned base commit and an identical measurement-series
ID, which includes tool, rule, runtime and normalization versions. Compatibility
also requires the consumer expectation artifact. Client drift requires the
generated artifact/digest and the contract digest used to generate it; a drift
boolean contradicting those digests is a measurement error. Generator versions
belong in the measurement series/tool contract and retained native artifact.

The base artifact's outer context identifies the current comparison run and head
source; `baseline.commit` identifies the historical contract input. This is
provenance validation over caller-retained inputs, not independent resolution of
Git objects, OpenAPI parsing or a generic proof that a collector's facts are true.
The synthetic files expose the field rename and collector facts for review.
Contract comparisons do not use historical-debt allowances or generic ratchet
flags. Existing ratchet evaluation remains available through `policy_engine.py`.

## Machine report

`harness-project-report/v1` retains the entire policy result and a `gates` table.
Each gate includes policy, subject, state, metric, base/head values, measurement
series, expected context, remediation classes and raw evidence links. Contract
gates also contain the relationship and contract provenance object. The base
contract input is retained in those links even for an absolute breaking-count
rule whose generic base metric value is null.

`indexes` maps component, subject, policy, status and relationship IDs to gate
table IDs. Gate IDs are deterministic positions within this report, not baseline
identities. Each component has `local`, `cross_component` and combined aggregates.
Components without selected gates report `not_applicable`. A contract appears in
both participants' indexes, but is counted only once in the project aggregate.
Project aggregate requiredness comes from policy and includes exception-review
errors from the original evaluator. Consumers must use the project aggregate for
project status; local component passes alone are insufficient.

## Reproducible CLI examples

Run from the repository root. The single Rust topology uses retained synthetic
Rust evidence to exercise configuration; real Rust shadow replay is documented
in the [reference adapter guide](rust-reference-adapter.md).

```bash
python3 tools/quality/project_report.py --reference-only check \
  --config tools/quality/fixtures/contracts/single-rust-config.json \
  --output target/quality/single-config.json
python3 tools/quality/project_report.py --reference-only evaluate \
  --config tools/quality/fixtures/contracts/single-rust-config.json \
  --evidence tools/quality/fixtures/contracts/single-rust-evidence.json \
  --expected tools/quality/fixtures/contracts/expected.json \
  --source-root tools/quality/fixtures/contracts/sources \
  --artifact-root tools/quality/fixtures \
  --output target/quality/single-project.json
```

Both commands exit 0. The Angular + Rust + Python + Java synthetic topology has
four passing local coverage gates, schema validity passing, and three contract
failures: one breaking response-field rename, stale generated client, and
incompatible consumer expectation. The next command intentionally exits **1**:

```bash
python3 tools/quality/project_report.py --reference-only evaluate \
  --config tools/quality/fixtures/contracts/project-config.json \
  --evidence tools/quality/fixtures/contracts/evidence.json \
  --expected tools/quality/fixtures/contracts/expected.json \
  --source-root tools/quality/fixtures/contracts/sources \
  --artifact-root tools/quality/fixtures \
  --output target/quality/polyglot-project.json
```

No collection tool is invoked. [Acceptance tests](../../tools/quality/tests/test_project_report.py)
exercise both commands and negative provenance cases. The
[validation record](gh-117-validation.md) records actual checks and limitations.

## Rust candidate migration

GH-149's independent `harness-gate-quality-core` candidate implemented generic
contract provenance checks and `project_report::report` under OpenSpec tasks
4.1–4.2. Its complete reports are compared with this Python reference, including
participant aggregates, lossless indexes and evidence links. See
[GH-149 validation](gh-149/README.md). GH-151 subsequently linked that core into
the released Rust package and accepted authoritative generic evaluation and
reporting as described above. Python shadow reports remain reference evidence.
