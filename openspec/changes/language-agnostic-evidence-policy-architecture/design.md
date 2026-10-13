# Design: Language-Agnostic Evidence and Policy Architecture

## Context

This design is reviewed after closure of the preceding quality-gate Issue series against
`main` at `ad54d8df6d21d3f6e3a0b5ee83918ae078a84d61`. At this baseline,
`ci_quality.py collect` is the required PR/push collection entry point for the original
coverage gate, expanded production coverage, the supported Rust function-risk ratchet,
and isolated critical-path evidence. `Required Quality Aggregate` is the stable required
check. The existing Rust risk series is deliberately conservative and does not certify
repository-wide CRAP for unmeasured production sources.

Motivation and authorization boundaries are defined in [proposal](proposal.md). Implementation and scoped Rust acceptance are now recorded in the [closure ledger](../../../docs/quality/architecture-closure.md). Conceptual examples below are not normative schemas; no real non-Rust adapter is certified.

Harness-Gate currently has strong Rust-first evidence semantics, including production-source ownership, raw line/function/region coverage, function risk/CRAP evidence, measurement series, critical-path traceability, negative fixtures and fail-closed behavior. Those semantics should be preserved. The purpose of this design is to separate them from the core so future ecosystems can participate without cloning or weakening policy behavior.

The preceding OpenSpec Issue series is complete. This design may now enter formal review and Issue decomposition, while implementation remains incremental and subject to the migration invariant below.

## Goals / Non-Goals

**Goals:** define stable core domain objects; define collector/evidence/policy boundaries; preserve raw measurement semantics; support multi-component repositories; make capability differences explicit; provide generic baseline/ratchet evaluation; enable AI-agent-readable remediation.

**Non-Goals:** no full Angular/Python/Java adapter in this change; no universal AST/build/coverage engine; no new branch baseline; no replacement of existing accepted Rust series; no unrestricted policy DSL.

## Decisions

### 0. Migration invariant: current required gates remain authoritative

Language-agnostic architecture is a migration of representation and policy orchestration,
not permission to reinterpret already-required evidence.

Until an explicit equivalence acceptance is merged:

- the current Rust collection/evaluation path remains authoritative for release decisions;
- normalized evidence is a projection of existing retained evidence wherever possible;
- shadow evaluation must use the same commit/base/run identities and compatible series;
- a generic-path mismatch is `measurement_error`/compatibility failure, never a reason to change the current Rust result;
- unsupported or intentionally limited Rust measurement remains explicit rather than being generalized into a false repository-wide capability;
- `Required Quality Aggregate` keeps its stable check identity and semantics;
- replacing an authoritative child gate requires a separately reviewable rollout step, not an incidental refactor.

This gives the migration a precise oracle: same evidence plus equivalent policy must produce the same machine outcome and debt classification.

### 1. The core models software components, not programming languages

The core entity hierarchy is:

```text
Project
  -> Component
      -> Target / SourceBoundary
          -> Subject
              -> Evidence / Metrics
```

A `Component` represents an independently analyzable technical unit such as a frontend, API, worker, CLI, library or contract package. It may declare language/framework metadata, but the core does not use that metadata to implement language-specific behavior.

Initial conceptual configuration shape (not accepted `flow.toml` syntax; see the GH-117 JSON placement decision below):

```toml
version = "1"

[project]
name = "example-platform"

[[components]]
id = "frontend"
path = "frontend"
language = "typescript"
framework = "angular"

[[components]]
id = "api"
path = "backend"
language = "rust"

[[components]]
id = "worker"
path = "worker"
language = "python"

[[components]]
id = "billing"
path = "billing"
language = "java"
```

Language/framework fields are descriptive and adapter-selection inputs, not core policy branches.

GH-111 implements tasks 0.3 and 1.1–1.4 as a standalone development model. The
[migration inventory](../../../docs/quality/migration-compatibility.md) freezes
current machine contracts; the [v1 model record](../../../docs/quality/project-model.md)
specifies the schemas, canonical identity, directed relationship graph and
explicit rename/move/split lineage. The TOML above remains an illustrative shape,
not a new project-local configuration contract (task 9.1 selected the opt-in JSON manifest; see the GH-117 decision below).

**Alternatives:** one Harness-Gate project per language would avoid a component model but cannot express cross-component contracts or project-level release evidence. A tool-centric model would couple the core to current ecosystems.

### 2. Subjects are generic analyzable entities with versioned identity

A `Subject` can represent `project`, `component`, `boundary`, `file`, `function`, `method`, `class`, `route`, `endpoint`, `contract`, `dependency`, `critical_path`, or another versioned subject kind.

The core never joins evidence only by short symbol name. Symbol-like subjects use a versioned identity containing enough source context to detect ambiguity and source drift. A reference shape is:

```json
{
  "identity_version": "subject-identity/v1",
  "component": "api",
  "kind": "function",
  "language": "rust",
  "path": "src/service/lease.rs",
  "qualified_symbol": "service::lease::LeaseManager::reconcile",
  "range": {
    "start_line": 82,
    "start_column": 5,
    "end_line": 143,
    "end_column": 6
  },
  "source_sha256": "..."
}
```

The conceptual design allowed reduced identity capability when source ranges or stable qualified symbols cannot be supplied. Executable v1 does not implement that negotiation: span is optional, but a source digest and unambiguous discriminator are required. The second-ecosystem mismatch register records this gap as TS-01. Ambiguous identities must fail closed; reduced identity requires a reviewed contract change.

Rename/move/split mapping is explicit baseline metadata; a new identity cannot automatically inherit favorable historical debt.

The initial executable schema uses versioned kinds such as `function/v1`,
repository-relative paths, a generic `discriminator` field and optional `span`.
Project, target and boundary references also participate in `subject-identity/v1`.
Its ID hashes canonical source identity and excludes descriptive metadata. The
example above is conceptual; the committed project-model schema is normative
for this standalone model. A successful lineage lookup does not accept a
baseline or implement generic debt/ratchet policy.

### 3. Collectors measure; they do not own release policy

The collector pipeline is:

```text
Harness collector request
  -> ecosystem/tool adapter
      -> raw tool output
          -> normalizer
              -> normalized evidence
```

Collectors may invoke or parse external tools, but their output is evidence. Final policy status belongs to the Harness-Gate policy engine.

A collector may report a tool-native status such as test process failure, parser error, or analyzer warning. The policy engine determines whether that status blocks the requested gate profile.

This separation prevents divergent rules such as one adapter considering 75% coverage acceptable while another considers 80% acceptable without an explicit project policy.

### 4. Normalized evidence uses a common envelope, not a common measurement algorithm

The existing `quality-evidence.schema.json` is a Rust complexity-evidence contract and must not be silently broadened in place into the universal schema. `harness-evidence/v1` is introduced alongside it; an adapter/projection preserves the old record and its series while adding generic project/component/subject/capability metadata.

The executable v1 [schema](../../../tools/quality/schema/harness-evidence.schema.json)
and [contract record](../../../docs/quality/harness-evidence.md) define the normative
shape. The [synthetic batch](../../../tools/quality/fixtures/harness-evidence/polyglot.json)
contains complete Rust/TypeScript/Python/Java examples. Metrics use discriminated
ratio/count/boolean/duration/size/decimal values, with exact rational CRAP added
by GH-116 to retain repeating native fractions. Ratios preserve covered/total
counts. Decimal and rational CRAP remain distinct series types. The full
project-model subject is embedded; provenance includes an explicit
base commit. Canonical serialization and filesystem integrity are validated before
consumption. This GH-112 implementation covers tasks 2.1–2.5 and 3.1–3.4 only;
Rust shadow projection is recorded below; scoped equivalence acceptance is recorded under GH-118 below; authoritative
gate replacement remains a separate rollout.

The common envelope standardizes provenance, identity, metrics and artifact linkage. It does not imply that Rust LLVM coverage, Java JaCoCo coverage, Python coverage.py and TypeScript Istanbul have identical semantics. Their measurement contracts and series remain distinct.

### 5. Capability states are explicit and non-numeric

Collectors declare capabilities per series and may report these states:

```text
supported
unsupported
not_configured
not_collected
measurement_error
not_applicable
```

A missing/unsupported branch metric must never become `0.0` or `1.0`. Policies can explicitly require support, allow informational unsupported status, or mark a component not applicable.

A collector claiming `supported` but failing to produce required evidence reports `measurement_error`, which is fail-closed for blocking policies unless an explicit policy says otherwise.

### 6. Measurement series protect semantic comparability

A measurement series identifies the meaning of a metric, not just a file format. Series identity includes the relevant collector/tool versions, rule versions, target/runtime assumptions, source identity rules and normalization semantics.

Examples:

```text
rust-function-risk-1
typescript-function-risk-1
python-function-risk-1
java-function-risk-1
```

The same metric key (for example `risk.crap`) may appear across series, but base/head ratchets require compatible series. If a tool or rule upgrade changes metric semantics, the adapter emits a new series and baseline acceptance is required before incremental comparison.

Existing accepted Rust series retain their historical meaning. Migration to the generic envelope wraps them; it does not rename them into a fictitious universal series.

### 7. Metric keys are generic; measurement contracts remain ecosystem-specific

Core metric namespaces include, but are not limited to:

```text
coverage.line
coverage.function
coverage.region
coverage.branch
complexity.cyclomatic
complexity.cognitive
risk.crap
mutation.score
mutation.killed
mutation.survived
security.findings
contract.breaking_changes
performance.regression
bundle.size
accessibility.violations
```

A policy references metric keys and scope. It does not parse JaCoCo, LCOV, coverage.py, Istanbul or mutation-engine output.

CRAP is therefore a cross-language policy metric when an adapter can provide compatible inputs under its own series:

```text
risk.crap = CC^2 * (1 - coverage_fraction)^3 + CC
```

The formula key can be shared while CC and coverage measurement rules remain series-specific.

### 8. Policies evaluate evidence independently of collectors

A constrained policy schema supports common comparisons, scope selection and aggregation. A conceptual configuration is:

```toml
[quality]
fail_closed = true
changed_code_ratchet = true

[quality.coverage]
metric = "coverage.line"
scope = "production"
minimum = 0.80

[quality.risk]
metric = "risk.crap"
scope = "changed-production-symbols"
maximum = 30

[quality.high_risk]
metric = "complexity.cyclomatic"
scope = "production-functions"
maximum = 10
```

Component-specific overrides are explicit. Tool adapters cannot silently change thresholds.

The v1 policy model intentionally supports a limited set of deterministic operations rather than arbitrary code execution or a user-defined programming language.

GH-114 implements tasks 5.1–5.5 through the standalone
[typed policy and result contract](../../../docs/quality/policy-engine.md).
All selectors expand caller-owned subjects in the requested target; no implicit
cross-subject averaging or language branch is introduced. Exact comparisons use
normalized types. Required non-pass states other than informational block, and
the aggregate retains original child causes, including cancellation. Optional
base evidence supplies remediation context; tasks 6.1–6.4 add explicit ratchet rules
as documented below.

### 9. Baseline and ratchet are core services

The baseline service compares compatible base/head evidence by subject identity and explicit identity mapping.

Required semantics include:

- unchanged historical debt remains visible;
- new subjects must satisfy the active policy;
- modified subjects may not hide behind historical exemptions;
- configurable no-regression policies compare base/head values;
- moved/renamed/split subjects require explicit mapping where inheritance is allowed;
- missing base or incompatible series blocks an incremental pass;
- stale or mismatched commit/target evidence is a measurement error.

The policy engine distinguishes "below absolute threshold" from "regressed relative to base" so projects can stage adoption without claiming legacy compliance.

GH-115 implements tasks 6.1–6.4 in the standalone shadow evaluator. The
[policy contract](../../../docs/quality/policy-engine.md) defines the closed
ratchet flags, exact comparison directions, derived debt ledger and independent
absolute-compliance fields. A source-digest/span edit uses an explicit one-to-one
`modify` mapping with an unchanged locator, extending the GH-111 mapping model.
Unmapped identities and split children receive new-subject treatment without debt
inheritance. Missing historical values or incompatible series prevent incremental
success. Separate exception metadata preserves the quality result and adds review
state; invalid/expired metadata blocks the combined aggregate. This implementation
does not accept the later Rust migration or alter current required gates.


### 10. Cross-component contracts are first-class subjects and gates

Project-level release confidence cannot be derived only by independently passing each language component.

Cross-component evidence includes relationships such as:

```text
Angular client <-> OpenAPI contract <-> Rust API
Java producer <-> AsyncAPI/event schema <-> Python consumer
migration <-> application model
```

Contract collectors may produce generic metrics/events such as:

```text
contract.breaking_changes = N
contract.client_drift = true/false
contract.schema_valid = true/false
```

A contract subject links all participating components and the exact contract artifact digest. Backend and frontend can both pass local tests while the cross-component contract gate still fails.

### 11. Gate results preserve reason, not only exit status

The standard result states are:

```text
pass
fail
warning
informational
skipped
unsupported
not_applicable
measurement_error
blocked
```

`fail` means valid evidence violates policy. `measurement_error` means the system cannot establish the required evidence. Both may block release, but they remain semantically distinct.

A machine-readable violation contains at least:

```json
{
  "gate": "changed-function-risk",
  "status": "fail",
  "component": "api",
  "subject": "...",
  "metric": "risk.crap",
  "base": 18.1,
  "head": 33.7,
  "limit": 30,
  "series": "rust-function-risk-1",
  "evidence": ["..."],
  "remediation_classes": ["reduce-complexity", "increase-meaningful-test-coverage"]
}
```

Human-readable reports are projections of the machine result, not the only source of truth.

### 12. Rust becomes the first reference adapter through migration, not rewrite

Existing Rust evidence is migrated in layers:

1. wrap current accepted evidence in the generic envelope;
2. add component/subject metadata without changing raw measurements;
3. route existing policy results through generic policy interfaces in shadow mode;
4. compare old and new outcomes byte-for-byte or semantically through compatibility fixtures;
5. only replace old required paths after equivalence is demonstrated.

No task may remeasure or reinterpret accepted historical Rust data solely to make it fit this architecture.

GH-116 implements tasks 7.1–7.5 with the standalone
[Rust reference adapter](../../../docs/quality/rust-reference-adapter.md).
It verifies and projects a retained candidate without subprocess collection,
then compares native production and risk decisions with generic policy results.
Native identity matching and debt rules compile into explicit generic policies;
the separate generic no-regression ratchet does not redefine Rust's existing
changed-function contract. Other required candidate stage statuses pass through
unchanged. Any mismatch blocks migration and the current Rust path stays
authoritative. Historical validation preserves the accepted Phase 1 series,
which predates this candidate format, and replays the unaccepted GH-97 candidate
without granting baseline approval. See the
[validation record](../../../docs/quality/gh-116-validation.md).

### 13. External adapters use a versioned collector protocol

The standalone GH-113 runner accepts internal callables and external stdin/stdout
adapters behind the same validated evidence interface:

```text
harness-gate -> collector request
collector -> raw artifacts + harness-evidence/v1
```

The protocol must carry project/component, commit/target, requested capabilities, workspace roots, output locations and policy-independent collection parameters.

The core validates returned evidence, artifact digests, declared series and capabilities before policy evaluation.

The [v1 collector contract](../../../docs/quality/collector-protocol.md) defines
the caller-bound request, exclusive evidence/error response, dedicated output
inventory, typed failures, and transport-independent return value. The
[synthetic fixtures](../../../tools/quality/fixtures/collectors/README.md) exercise
both transports using retained normalized/raw evidence. This implements tasks
4.1–4.4 only; authoritative Rust collection and later policy/migration tasks remain
unchanged. Internal execution is trusted code; external execution is not an OS
sandbox. POSIX subprocess deadlines terminate the process group.

### 14. Security and trust boundaries remain fail-closed

Collectors are not trusted merely because they return JSON. Evidence validation includes schema, allowed paths, artifact digests, commit/target consistency, duplicate/unknown subject detection and series compatibility.

Where collectors execute third-party tools, their command provenance and versions are recorded. A future plugin signing/trust model may be added, but this change does not create one.

### 15. Rollout uses shadow mode, existing raw evidence, and compatibility evidence

The architecture is introduced without immediately changing required gates:

```text
existing required Rust gate
        +
new generic shadow evaluation
```

Both run from the same raw evidence where possible. Any disagreement blocks migration of that gate and produces a compatibility report.

After Rust equivalence is accepted, a later non-Rust adapter (recommended: TypeScript/Angular because it exercises frontend-specific needs and cross-component contracts) validates that the abstraction is genuinely language-agnostic.

The architecture is not declared stable merely because Rust successfully uses it.

## Risks / Trade-offs

- **Over-generalization** -> constrain v1 concepts to requirements already demonstrated by Rust plus one planned non-Rust adapter; avoid speculative universal AST concepts.
- **Semantic flattening** -> preserve per-series measurement contracts and raw evidence; normalize envelopes, not meaning.
- **Migration regression** -> shadow old/new paths and require equivalence fixtures before replacement.
- **Plugin sprawl** -> keep collector protocol minimal and require explicit capabilities/versioning.
- **Policy complexity** -> no unrestricted DSL in v1; use deterministic schema and typed operators.
- **CI cost** -> reuse raw evidence and avoid duplicate expensive runs during normalization.
- **Identity instability** -> version identity rules and require explicit mappings for rename/move/split.
- **False portability claim** -> architecture acceptance and ecosystem certification are separate statuses.

## Migration Order

```text
preceding Issues closed
  -> freeze architecture docs
  -> generic project/component/subject model
  -> evidence envelope + capability model
  -> policy/baseline/ratchet core
  -> Rust compatibility wrapper/reference adapter
  -> shadow CI equivalence
  -> accept architecture v1
  -> TypeScript/Angular reference adapter in a separate change
  -> Python/Java adapters only after abstraction survives real second-ecosystem use
```

## Open Questions for Implementation Review

1. Whether the generic evidence envelope should live in the current quality schema namespace or a new top-level `schema/` namespace.
2. Whether component declarations extend the current Harness-Gate configuration schema or live in a quality-only manifest first.
3. Which current Rust policy code can be generalized without changing accepted report contracts.
4. Whether external collectors are subprocess-only in v1 or may also be linked/internal adapters behind the same interface.
5. Which TypeScript/Angular project will serve as the first non-Rust validation fixture after this change is accepted.


### GH-117 configuration and reporting decision (tasks 8.1–8.3, 9.1–9.3)

Use an opt-in `.harness-gate/project.json` manifest (`harness-project-config/v1`)
with explicit repository-relative model and policy references. Keep component
and relationship declarations in the referenced `harness-project/v1` document.
The standalone Python CLI resolves references against an explicit project root;
unknown versions/keys and escaping paths fail. Rust `flow.toml` parsing, default
paths, source boundaries and release authority remain unchanged.

Relationship-scoped policies select provider-owned contract subjects and validate
retained head/base contract, consumer expectation and generated-client provenance
before generic metric comparison. Exact subject scopes support local gates.
The report keeps one lossless gate table with component/subject/policy/status/
relationship indexes and separate local/cross-component aggregates. Shared gates
appear under both participants but count only once at project level.

The [implementation contract and executable examples](../../../docs/quality/project-reporting.md)
record metric semantics, baseline trust boundaries and additive compatibility.
The four-language topology is synthetic; no second reference ecosystem or
OpenAPI tool adapter is certified. The [validation record](../../../docs/quality/gh-117-validation.md)
covers this scoped implementation.

## GH-118 shadow rollout and scoped equivalence acceptance

The advisory `Generic Quality Shadow` job depends only on the existing Rust
collection job. It downloads that run/attempt artifact and projects the original
base/head/run context without another collection. Upload preserves normalized
evidence, policy and compatibility reports even on failure. The required
aggregate keeps its name, dependencies and fail-closed behavior. Rust remains
the sole release authority.

The [pinned acceptance window](../../../docs/quality/rust-equivalence-acceptance.md)
requires both GH-96 and GH-97 retained runs at two distinct head commits, plus
all negative adapter fixtures. This retrospective window passed locally with no
unexplained outcome, debt, unsupported-state or measurement-error differences.
It does not claim a hosted soak or certify additional series or ecosystems.
[ADR-0040](../../../docs/adr/0040-language-agnostic-evidence-policy.md) records
noninterchangeable series, collector/policy separation and the fail-closed trust
boundary. The [validation record](../../../docs/quality/gh-118-validation.md)
covers tasks 10.1–10.4; their merged PR and successful required hosted CI are
linked in the GH-119 closure ledger. Final GH-119 CI and controller acceptance
remain pending. Any authority replacement requires a separate reviewed rollout.

## GH-119 closure and second-ecosystem boundary

The [closure ledger](../../../docs/quality/architecture-closure.md) reconciles
submission-time statuses with accepted predecessor deliveries. The separate
[TypeScript/Angular design](../typescript-angular-reference-adapter/design.md)
selects a real fixture plan and records TS-01–TS-03 mismatches and open integration
questions. It does not implement or enable an adapter. Generic v1 closure does
not certify ecosystem stability or authorize required-gate replacement.
