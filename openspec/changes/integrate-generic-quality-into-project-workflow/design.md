# Design: Integrate Generic Quality into Project Workflow

## 1. Governing invariants

This design inherits `docs/engineering-policy.md`, ADR-0039, ADR-0040, the released Rust generic-core authority transfer, and the completed CI topology contract.

The following are fixed inputs, not design choices in this change:

- collectors measure; the released Rust core decides;
- required gates fail closed;
- unsupported/absent/error capability states are not favorable numeric measurements;
- accepted Rust CRAP/coverage/risk series and thresholds are unchanged;
- debt/no-regression and baseline lineage semantics are unchanged;
- provenance-sensitive evidence has one collection owner per series;
- retained authoritative artifacts require immutable identity/provenance validation;
- `hook` may be cheaper than `full`/`ci`, but omission cannot become PASS;
- currently required hosted platform assurance is not reduced here.

## 2. Two-plane configuration

### 2.1 Execution plane: `flow.toml`

`flow.toml` remains schema v2 for this change unless a narrowly required compatible field is proven necessary. It owns:

- project execution defaults;
- paths/aliases;
- doctor checks;
- services;
- traditional parsers;
- steps and dependencies;
- scope rules;
- retries/shards/notifications;
- command execution profiles.

It does not become the source of generic quality thresholds, collector capability truth, project relationships, or ratchet policy.

### 2.2 Quality/control plane: `quality.toml`

Introduce `.harness-gate/quality.toml`, schema version 1. The conceptual model contains:

- project identity and quality-model version;
- components and source/artifact boundaries;
- subjects or subject-discovery bindings;
- component relationships/contracts;
- collector bindings and declared protocol/capability expectations;
- policy bindings to accepted metric/capability series;
- profile participation (`hook`/`full`/`ci` or declared equivalents);
- baseline provider strategy;
- exception/selection references where supported;
- report/output intent.

The schema should prefer references to versioned machine contracts over embedding language-specific tool options. Tool-specific execution details belong in collector configuration/adapter request material, not generic policy semantics.

The bounded tasks 1.1–1.4 implementation is specified in
[quality configuration v1](../../../docs/quality-configuration.md) and
[ADR-0041](../../../docs/adr/0041-quality-configuration-v1.md). It binds existing
`harness-policy/v1` rule files rather than duplicating requiredness or ratchet
settings in collector declarations. `config check/print` validate both planes;
`schema export --quality` exports the independent schema. Compilation and verify
orchestration are layered on this model. Schema export, `generic` and flow-only
migration never create implicit quality configuration. New reference presets
explicitly compose it as described in section 8.

### 2.3 Authority constraints in schema validation

`quality.toml` must not permit a collector to define final requiredness or PASS/FAIL. A collector binding may state what capability/series it is expected to produce and in which profiles it is invoked. Policy bindings determine whether the capability is required and how it is evaluated.

Configuration validation rejects contradictions such as:

- required policy references a capability with no expected producer in a profile where it must be enforced;
- a preset promises certified CRAP while binding an uncertified/incompatible series;
- component/subject/relationship references do not resolve;
- baseline strategy cannot provide identity required by a ratchet policy;
- collector tries to override requiredness/threshold/aggregate authority;
- `hook` claims a required expensive measurement passed while not collecting/using valid retained evidence.

## 3. Compilation to trusted low-level inputs

Do not create a second policy engine in `verify`. The orchestration layer compiles validated configuration/runtime state into the existing low-level trusted contracts consumed by the Rust evaluator:

```text
quality.toml + repository state + verified collector outputs
        |
        +--> harness-project/v1
        +--> harness-policy/v1
        +--> harness-evidence/v1[]
        +--> trusted expected context
        +--> source/artifact roots
        +--> selection/mappings/exceptions when applicable
        +--> compatible baseline context
        |
        `--> Rust quality evaluator
```

The compiler is deterministic and testable. Its GH-180 transport and identity rules are specified in [the compiler reference](../../../docs/quality-compilation.md) and [ADR-0042](../../../docs/adr/0042-trusted-quality-compilation.md). Ecosystem identifiers and kind aliases are pack/configuration data; the generic compiler has no closed language or framework dispatch enum. `quality evaluate` remains the direct form of the same evaluation boundary. Given semantically equivalent compiled inputs, `verify` and direct `quality evaluate` must produce equivalent generic decisions/reports.

## 4. Verify orchestration

Extend `verify` with explicit phases while preserving traditional gate behavior:

1. Discover project root and load/validate flow + quality configuration.
2. Establish source identity and selected profile/scope.
3. Resolve affected components/subjects.
4. Run secrets/audit/traditional configured steps according to flow semantics.
5. Build trusted collector requests for quality bindings applicable to the profile/selection.
6. Execute collectors through the existing signed/out-of-process adapter host or an equivalent trusted in-process reference boundary where already accepted.
7. Validate collector protocol, capability state, source/tool/config identity, artifact manifest, and evidence schema.
8. Resolve baseline provider and validate compatibility/lineage.
9. Compile trusted generic evaluator inputs.
10. Invoke the released Rust generic quality core.
11. Combine workflow execution status and generic project quality into one final verify result/report without allowing a traditional step to override generic policy semantics.

A required failure in either the traditional execution plane or authoritative generic quality plane blocks `verify`.

## 5. Collector orchestration

### 5.1 Binding model

A collector binding identifies:

- stable collector id;
- adapter/protocol identity and compatible version range;
- component/subject discovery boundary;
- capabilities/measurement series expected from the collector;
- execution profile participation;
- trusted executable/signature/package identity configuration;
- required input/artifact roots;
- resource/time limits supplied to the adapter host where already supported.

The binding does not contain policy verdicts.

### 5.2 Capability honesty

Collectors return supported/unsupported/not-collected/error states according to the protocol. The orchestration layer validates expected capability contracts. It never synthesizes CRAP from arbitrary complexity and coverage outputs merely because both exist.

For the current Angular/TypeScript certification, CRAP remains unsupported. Rust presets may require CRAP only for the accepted Rust measurement series/boundary.

### 5.3 Failure behavior

Required expected evidence fails closed on:

- collector launch/signature/protocol failure;
- timeout/crash/non-zero protocol failure;
- missing expected capability;
- malformed evidence;
- source/config/tool identity mismatch;
- stale/mixed artifact manifest;
- incompatible measurement series;
- duplicate/conflicting authoritative producer for the same series/subject.

## 6. Baseline providers

Normal users should not supply `--base-evidence`, `--base-project`, `--base-expected`, `--base-source-root`, and `--base-artifact-root` manually.

Introduce provider interfaces that materialize the same trusted base context.

GH-182 implements this stage as `quality baseline`; `verify` integration remains
task 5. Both transports take independently host-pinned base state and manifest
digest, validate exact bundle inventory/provenance, and export the existing five
base inputs. Ecosystem and capability names remain opaque during transport;
strict evaluator metric support and debt decisions remain unchanged. See
`docs/quality-baselines.md` and ADR-0044 for the versioned request/manifest contract,
Git representation limits and optional-baseline behavior.

### 6.1 Git base-ref provider

A deterministic provider may resolve a configured base ref (for example the merge base with `origin/main`) into an isolated source snapshot and collect/load compatible base evidence. It must bind exact commit identity and must not mutate the developer working tree.

Collection cost is profile-aware. Cached/retained base evidence may be reused only with validated source/config/tool/series identity.

### 6.2 Retained CI artifact provider

CI may resolve a trusted retained artifact for an exact accepted base commit/config/series. Artifact manifests/hashes are validated according to the CI topology contract. Missing or mismatched required base material does not silently reset the ratchet.

### 6.3 No-baseline behavior

Policy explicitly determines when a baseline is optional, unavailable-but-reviewable, or required. The provider cannot convert missing baseline into a fresh favorable baseline. Existing Rust debt/ratchet semantics remain authoritative.

## 7. Profiles and cost

### Hook

- traditional fast deterministic gates;
- cheap collector capabilities may run;
- full coverage/CRAP collection is not required solely for hook latency;
- omitted measurements are represented honestly and do not create authoritative full-quality PASS.

### Full

- normal complete local verification;
- certified required measurements run, including Rust CRAP/coverage/risk where applicable;
- baseline/ratchet runs when required by policy;
- unified report is authoritative for the local full profile.

### CI

- same generic semantics as full plus hosted/trusted artifact/baseline context as configured;
- reuse existing quality-coverage ownership instead of launching a second equivalent collection because `verify` exists;
- integration should consume validated retained evidence where CI already has the authoritative producer;
- no new redundant full collector job without explicit cost/evidence justification.

## 8. Presets

Upgrade at least:

- `rust-api`;
- `angular-only`;
- `angular-rust-postgres`.

Each preset generates coherent `flow.toml`, `quality.toml`, audit/secrets configuration, and documentation hints.

### Rust API

Default full/CI quality binds the accepted Rust risk/coverage/CRAP series. CRAP is required according to Engineering Policy and accepted measurement boundary. Hook does not need to recollect full CRAP.

### Angular only

Generate supported Angular/TypeScript collector/policy capabilities from the accepted certification. CRAP is explicitly unsupported unless a future certification changes that fact.

### Angular + Rust + PostgreSQL

Model frontend/backend components and their relationship, execute ecosystem-specific collectors, and aggregate local plus cross-component quality into one project report. PostgreSQL remains an execution/service concern unless a quality capability explicitly requires it.

The generic preset omits `quality.toml` and preserves any existing quality file.
Flow-only migration remains execution-only. Adopters generate a reference in a
separate directory and review/copy quality and policy files, align profiles and
project identity, then provision trusted runtime inputs before verification.

Reference combinations are UX, not core architecture. Declarative recipes select
reusable ecosystem packs and independent capability packs with target/path
bindings. Generic composition only expands, merges and validates data; collectors,
series, policy and capability states live in pack/config data. Conflicting output
and path escapes fail before writes. PostgreSQL contributes no implicit quality.
A future `frontend=vue`, `backend=go`, `database=postgres` selection can compose
packs without language-pair branches or redesigning the generic schema/compiler/
verifier. Built-in catalog/flow templates are reference data; registry discovery,
installation, version resolution and execution-pack composition remain future work.
A synthetic unregistered ecosystem pack exercises the same composer and validator.

Generated metadata pins accepted reference series; it cannot authenticate evidence
or produce runtime state/signatures. Host provisioning and explicit series/baseline
migration are required for actual installed toolchains. Baseline starts at `none`,
with no implicit debt waiver. See ADR-0047 and `docs/quality-presets.md`.

## 9. Reporting

`verify` should produce a stable machine result that links:

- selected source/profile/scope;
- traditional secrets/audit/step outcomes;
- collector execution/capability summaries;
- evidence/artifact identities;
- component/local quality results;
- cross-component contract results;
- baseline/ratchet/debt state;
- exceptions/review context;
- authoritative generic project report;
- final combined verify status.

Human output should answer: what failed, where, why, base vs head where relevant, evidence link/path, and remediation context. CRAP failures should expose subject, head/base CRAP, threshold/ratchet state, and supporting coverage/complexity evidence already required by the accepted contracts.

The machine contract must distinguish execution failure from quality-policy failure while still yielding one blocking final status.

## 10. Backward compatibility and migration

Existing repositories with only `flow.toml` continue to use current verify semantics. `quality.toml` integration is activated by explicit file presence/preset migration rather than silently imposing new collectors on legacy repositories.

Provide validation/migration guidance rather than auto-generating policy that could surprise an existing project. Preset initialization for new projects may include quality by default where the preset contract promises it.

Direct `quality evaluate` and `adapter run` remain supported advanced/debug interfaces. `compat` lifecycle is outside this change.

## 11. CI integration

The completed CI topology is normative for implementation:

- pinned tool acquisition remains explicit;
- actual Cargo target/cache boundaries are respected;
- provenance-sensitive series have one collection owner;
- retained artifacts are immutable and identity/hash validated;
- ineffective compiled-cache/build-once optimizations are not reintroduced without new hosted evidence;
- Required Quality Aggregate remains lightweight and stable;
- hosted platform requirements remain unchanged.

## 12. Rollout

Implement in stages with differential/equivalence tests:

1. quality schema/model and validation;
2. deterministic compiler to low-level trusted inputs;
3. collector orchestration and evidence validation;
4. baseline providers;
5. verify integration/reporting;
6. presets/migration/docs;
7. CI reuse and hosted acceptance;
8. real project dogfood after this OpenSpec.

At each authority-sensitive stage, compare direct low-level Rust evaluation with the `verify`-compiled path. Rollback disables/removes the integration layer while retaining low-level Rust authority and evidence; it must not fall back to Python or collector-owned decisions.
