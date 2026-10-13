# Design: Project-Owned Validation Boundary and Arc-Admin Dogfood

## GH-208 authority-transfer disposition

The [final recommendation](../../../docs/dogfood/arc-admin/decision/README.md)
is **NO TRANSFER**: retain `cargo flow` required authority and bounded shadow mode.
GH-207 establishes bounded 27-result full-command parity, but native quality
remains blocked. Native trust/lineage integration, hook/environment/service/CI
parity, native negatives and rollback rehearsal, and complete-quality/topology
cost acceptance remain exact blockers tracked in GH-215. The recommendation
retains migration-effort, parity, negative, cost and product-gap receipts plus
the rollback boundary. Repaired historical gaps are distinguished from current
blockers; unmeasured work and costs remain unknown.

Harness-Gate Result is validation evidence, not application/workflow lifecycle
state. No existing infrastructure is removed. Task 9.3's safe-transfer condition
is false; any future removal/consolidation requires a separate accepted change.
Evidence retention and strict validation do not accept the entire proposal:
task 9.4 closure remains withheld, as do the separately scoped pending policy/ADR
tasks. Required Quality Aggregate must be green through controller verification.

## GH-204 observation record

Tasks 5.1–5.4 are evidenced by the [shadow matrix](../../../docs/dogfood/arc-admin/shadow/README.md). Arc-Admin at the pinned/current revision passed all 25 command gates plus secret/audit prelude. Complete Harness-Gate loading rejects the unchanged import's unordered shared PostgreSQL consumers, both with and without quality composition. This is a capability gap; the narrower success of structural import/quality-binding validation is a separate UX gap. No execution order or blocker was edited to obtain a PASS.

An observation can complete with classified blocked comparisons. Unselected/unexecuted gates must retain NOT_RUN, and component selection stopped before computation must retain NOT_COMPUTED. Neither declaration parity nor another engine's existing report establishes runtime parity. Source identities, configuration hashes, command exits, native reports, service observations and explicit isolation limits accompany the matrix. Harness-Gate validation results have no workflow/lifecycle or merge authority. Remediation, controlled negatives and authority-transfer decisions remain in tasks 6–9.

## GH-206 cost and ownership design

The [cost record](../../../docs/dogfood/arc-admin/cost/README.md) and reproducible ledger map all 25 existing command hooks and nine declared quality capabilities to single proposed owners. The target uses one execution plan, one producer per compatible supported measurement identity, immutable artifact fan-out, authenticated baseline reuse and evaluation-only aggregation. Project commands, prelude semantics, service isolation, CI-only/security checks, profile requiredness and accepted quality policies remain obligations. Angular CRAP remains unsupported. No gate is removed and no replacement is deployed.

The before self-hosted CI sample records 799 seconds execution span and 1,077 runner-seconds. GH-204 only supplies local step timers and pre-dispatch failures; neither added shadow CI cost nor target savings is measured. Task 7.1 remains open pending matched self-hosted trials after task-8 remediation and host provisioning. Tasks 7.2–7.4 establish the inventory, conditional topology and retention hold, not successful parity or authority transfer. See the cost record for attribution formulas, exact remaining evidence and the assurance mapping to ADR-0040/ADR-0044.

## 1. Durable architecture boundary

Harness-Gate is a gate orchestration, evidence ingestion and quality-decision platform. It is not an application test framework.

The product SHALL expose three generic extension layers:

### 1.1 Command hook / execution gate

Use for project-owned validations whose primary contract is execution success/failure plus optional structured output. Examples include:

- unit/integration/API/E2E/smoke/load/migration tests;
- code generation consistency;
- schema/deployment/observability/supply-chain validation;
- project-specific scripts and organization-specific checks.

Harness-Gate owns only generic execution semantics: selection/scope, profiles, dependencies, services, environment, timeout, retry, requiredness, logs/artifacts and report linkage. The project owns the executable/test code and its domain-specific meaning.

### 1.2 Structured result adapter

A command gate MAY attach a declared structured-result parser/adapter. Generic reusable formats such as JUnit, SARIF or versioned JSON contracts may be supported. Parsed details enrich diagnostics and machine reporting but SHALL NOT silently turn tool-native severity or test metadata into Harness-Gate policy authority unless an explicit generic policy contract governs that field.

### 1.3 Quality collector plugin

Use when measurements need generic cross-run/project policy such as coverage, complexity, CRAP inputs, bundle size or performance series. Collectors produce normalized evidence under existing trust/identity rules. The released Rust core owns requiredness, thresholds, baseline/ratchet/debt and final generic quality decisions.

## 2. Anti-specialization rule

Adding a new project test runner or application validation framework SHALL NOT require a Harness-Gate Core code change if it can already be represented as a command hook.

Examples:

```text
Playwright E2E     -> command hook
Cypress E2E        -> command hook
Postman/Newman API -> command hook
Go integration     -> command hook
pytest API         -> command hook
k6 load smoke      -> command hook
custom shell test  -> command hook
```

Harness-Gate-specific support is appropriate only for reusable protocol surfaces: structured result formats, collector protocols, ecosystem/capability packs, service orchestration or policy semantics.

## 3. Arc-Admin baseline/oracle

Arc-Admin currently owns a mature `.arc-flow/flow.toml` and `cargo flow` binary. Before migration, freeze:

- all required step IDs;
- component/scope mappings;
- hook/full behavior;
- PostgreSQL service requirements;
- environment removals/injections;
- parsers/timeouts;
- CI job routing and self-hosted execution;
- pre-commit and delivery expectations;
- existing project-specific gate implementations.

The frozen inventory is the assurance oracle. A Harness-Gate migration may be stricter, but it must not omit an existing blocker without explicit reviewed rationale.

GH-201 freezes the [Arc-Admin before state](../../../docs/dogfood/arc-admin/README.md) at commit `9982ed556eaf997910824d7b682946147c81a16a`, with hash-checked source snapshots, all 25 blocking steps, hook/full semantics and reproducible historical self-hosted CI timing/artifact metadata. The accompanying unknown-runner regression and Engineering Policy consistency anchors enforce the configuration-driven extension boundary. This evidence completes only tasks 1.3 and 2.1–2.4; it does not establish import support, shadow parity, generic quality configuration or authority transfer.

## 4. Migration/import strategy

Arc-Admin's execution-plane config is structurally close to Harness-Gate flow schema. The dogfood SHALL prefer a deterministic import/migration mechanism over hand-copying steps.

The migration result must preserve semantic identity where possible:

```text
.arc-flow/flow.toml
       |
       +-- aliases/services/parsers/scope/steps/profiles
       v
.harness-gate/flow.toml
```

Any unsupported field or behavior must be reported explicitly. The importer SHALL NOT silently drop required steps or coerce unsupported semantics into defaults.

Manual edits after import should be limited to documented incompatibilities or intentional Harness-Gate additions. Migration effort is a product metric and must be recorded.

GH-202 implements `harness-gate config import --execution-only` before project
configuration discovery. It reads Arc-Flow v2 TOML without environment expansion,
validates the compatible projection through the existing execution model, and
rejects unknown or lossy declarations before publishing configuration and a
deterministic JSON parity/UX report. Existing outputs are never overwritten.
All source declarations and step order are retained; explicit dependencies use
the existing dependency validator. Arc's omitted secret path is materialized and
each command receives `input = "repository"` to retain working-tree access.

Full migration fails visibly without the explicit execution-only boundary:
global Arc environment overrides, audit/secret prelude behavior, services and
CI/hook integration still require project work and shadow evidence. The report
always blocks authority transfer; no quality policy or `ci` profile is invented.
The [import record](../../../docs/dogfood/arc-admin/import/README.md) compares all
25 selected-step blockers against GH-201, including the two outside the policy
list. Its metrics count one import command, zero re-entered steps/config edits,
491 preserved scalar values and 25 duplicated step definitions across the retained
source and generated configuration. Human elapsed time and runtime integration
effort remain explicitly unmeasured. This implements tasks 3.1–3.4 only and does
not establish runtime parity or acceptance of this complete proposal.

## 5. Quality configuration

Add `.harness-gate/quality.toml` separately from migrated execution config.

Arc-Admin should model at least:

- `frontend` Angular/TypeScript component;
- `backend` Rust component;
- relevant frontend/backend API relationship/contract;
- certified Rust quality collector/series;
- Angular certified capabilities with CRAP explicitly unsupported;
- trusted baseline provider;
- full/ci quality participation and honest hook omission where expensive.

Project-owned E2E/API/smoke steps remain in `flow.toml`; they do not become fake collector capabilities merely to appear in the generic quality report.

GH-203 supplies the reviewable [Arc-Admin quality configuration](../../../docs/dogfood/arc-admin/quality/README.md), composing unchanged Rust, TypeScript and API-contract packs. It binds the certified Rust series and required Git merge-base baseline provider, while keeping Angular CRAP unsupported. The pinned GH-202 flow declares `full` and `hook`, so quality uses those profiles and does not invent `ci`. Host-provisioned subjects, signed requests, trusted keys and baseline evidence remain runtime prerequisites; this configuration does not claim completed Arc-Admin runtime integration or shadow parity.

## 6. Dogfood phases

### Phase A — observation

No Arc-Admin required authority changes. Generate/import Harness-Gate configuration and run config/schema checks. Record setup friction and unsupported semantics.

### Phase B — shadow parity

Run existing `cargo flow` and Harness-Gate against equivalent source states. Harness-Gate remains non-authoritative for Arc-Admin merge policy. Compare:

- selected components;
- required command-gate results;
- service behavior;
- failure classification;
- diagnostics/artifacts;
- elapsed/runner cost;
- duplicate command/measurement execution.

### Phase C — generic quality addition

Enable certified Rust coverage/CRAP/baseline/ratchet through Harness-Gate. Keep Angular CRAP unsupported. Verify generic quality adds value without altering project-owned command semantics.

### Phase D — controlled negatives

Use temporary/dedicated branches or fixtures, never permanent broken main code, to prove:

- E2E/API/smoke command failure blocks when required;
- collector failure/missing evidence fails closed;
- incompatible/stale baseline cannot reset lineage;
- Rust CRAP regression blocks;
- unsupported Angular CRAP is not invented;
- scope/profile omission is represented honestly.

GH-205 retains the [controlled negative corpus](../../../docs/dogfood/arc-admin/negative/README.md), with paired CLI command controls, signed synthetic quality fixtures and explicit expected failure diagnostics. Frozen application/configuration sources remain unchanged. This phase proves generic boundaries without claiming native Arc-Admin measurements or resolving the phase B shared-service/provisioning gaps; full proposal acceptance and authority transfer remain pending.

### Phase E — authority-transfer decision

Only after accepted parity and cost evidence may a follow-up change transfer Arc-Admin required authority to Harness-Gate and remove duplicated old infrastructure. This OpenSpec may recommend that follow-up but SHALL NOT silently delete `cargo flow` as part of initial dogfood.

## 7. Gap classification

Every dogfood discrepancy must be classified before modification:

- **Arc-Admin project issue** — invalid project test/config/code under existing intended policy;
- **Harness-Gate capability gap** — generic execution/result/evidence model cannot express an existing valid gate;
- **Harness-Gate UX gap** — capability exists but migration/configuration/diagnostics are unnecessarily difficult;
- **expected semantic difference** — Harness-Gate intentionally adds stricter generic quality under accepted policy.

A capability/UX gap is fixed or tracked in Harness-Gate. Arc-Admin SHALL NOT weaken or delete an existing valid gate merely to achieve parity.

## 8. CI cost and duplication

During shadowing, duplication may temporarily exist for measurement, but it must be visible and bounded. No permanent final topology should run equivalent authoritative measurements twice.

The dogfood record must distinguish:

- project command work;
- measurement collection work;
- result parsing/report work;
- setup/tool acquisition;
- self-hosted runner wall time.

Authority-transfer recommendations should identify which old jobs/steps can be removed only after Harness-Gate ownership is proven.

## 9. Long-term ecosystem implication

This boundary is intentionally ecosystem-neutral. A future Go + Vue project should be able to keep its Go tests, Vue/Playwright E2E, API tests and custom scripts project-owned while Harness-Gate provides generic command hooks and separately binds certified Go/Vue collectors/packs. No application test framework belongs in Generic Core solely to make its command executable.


### GH-206 remediation and bounded CI follow-up

[ADR-0050](../../../docs/adr/0050-serial-shared-service-ordering.md) recognizes guaranteed single-worker dispatch for same-service consumers while retaining parallel conflict checks. [Three self-hosted workload pairs](../../../docs/dogfood/arc-admin/cost/selfhosted.md) now execute the unchanged imported flow: every one of the 25 project steps and two preludes matches and passes. Historical GH-204/205 receipts remain unchanged. This resolves HG-CAP-001 for serial dispatch; it does not establish complete runtime quality parity.

GH-207 must still address trusted native collector/state/key and baseline provisioning. The actual `QUALITY_BLOCKED` failure for missing `.harness-gate/runtime/full-state.json`, and the null profile name in that early-failure diagnostic, are retained. Complete-quality cost and original-topology savings remain unknown; no authority transfer or gate removal is permitted from the bounded cost series.

### GH-207 product remediation

The [gap ledger and rerun](../../../docs/dogfood/arc-admin/remediation/README.md) address tasks 8.1–8.4. Offline execution import uses the same static semantic/resource validator as production loading, without applying environment overrides or executing commands. This closes the resource-validation mismatch while preserving deterministic, lossless imported bytes. Production `config check` and `scope` are separate host-readiness checks; only workflow execution can establish native command/evidence outcomes.

Quality preparation now initializes diagnostics with the requested profile, so missing trusted inputs report `full` rather than null. The failure remains blocked/configuration with no invented policy participation or project report. Generic regressions cover conflicting import logs, valid serial import and missing workflow state/keys. ADR-0049's project-owned validation boundary and ADR-0050's serial-service rule remain unchanged.

[GH-215](https://github.com/musutrade/Harness-Gate/issues/215) explicitly tracks native producer/state/key/baseline provisioning, remaining hook/host/routing observations and complete-quality/original-topology cost. These require project/host integration and authentic native evidence; no application framework is promoted into Generic Core and no synthetic negative fixture is substituted for native measurement. Historical observations remain immutable. Task 9 and complete proposal acceptance remain pending.
