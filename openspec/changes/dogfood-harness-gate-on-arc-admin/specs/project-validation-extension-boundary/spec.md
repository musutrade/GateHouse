# Project Validation Extension Boundary

## ADDED Requirements

### Requirement: Authority-transfer recommendations SHALL preserve evidence and withhold unsafe closure

The dogfood recommendation SHALL explicitly decide transfer or no transfer using
retained migration-effort, parity, controlled-negative, CI-cost and product-gap
evidence and an explicit rollback boundary. Unknown measurements SHALL remain
unknown. When transfer is unsafe, existing required authority and bounded shadow
mode SHALL remain, exact blockers SHALL be identified, and proposal closure SHALL
be withheld. Safe transfer SHALL require a separate follow-up change for removal
or consolidation of old workflow infrastructure. Harness-Gate Result SHALL remain
validation evidence, not an application/workflow lifecycle state source.

#### Scenario: Traditional parity succeeds while native quality remains blocked
- **GIVEN** retained full-workload command parity and synthetic negative evidence
- **AND** missing trusted native quality inputs and incomplete host/routing/cost evidence
- **WHEN** the authority-transfer recommendation is produced
- **THEN** it records no transfer with exact blockers and a rollback boundary
- **AND** retains existing gates, requiredness, thresholds and baseline/ratchet lineage
- **AND** does not infer native quality success or lifecycle state from Result
- **AND** retains evidence and strict validation without closing the unsupported proposal

#### Scenario: Later evidence supports safe transfer
- **GIVEN** accepted parity, native fail-closed quality, complete cost and rollback evidence
- **WHEN** a subsequent recommendation supports transfer
- **THEN** removal or consolidation of old infrastructure requires a separate follow-up change
- **AND** the initial dogfood does not silently delete existing workflow infrastructure

### Requirement: Project-owned validation SHALL remain project-owned

Harness-Gate SHALL treat application- and repository-specific test or validation implementations as project-owned behavior. The project SHALL remain responsible for test code, domain assertions, fixtures, request flows, browser automation, load scenarios, migration checks, generation checks and other application-specific validation logic.

Harness-Gate SHALL provide generic orchestration and result/evidence integration rather than reimplementing project test frameworks.

#### Scenario: Project uses Playwright for E2E
- **GIVEN** a project owns Playwright E2E tests
- **WHEN** the tests are configured as a required Harness-Gate execution step
- **THEN** Harness-Gate executes and gates the project command through generic execution semantics
- **AND** Harness-Gate does not require a Playwright-specific Generic Core implementation
- **AND** the Playwright test logic remains in the project repository

#### Scenario: Project changes E2E framework
- **GIVEN** a project replaces Playwright with another executable E2E runner
- **WHEN** the replacement can be expressed using the existing command-hook contract
- **THEN** the project can update configuration without modifying Harness-Gate Generic Core

### Requirement: Harness-Gate SHALL expose a generic command-hook execution layer

Harness-Gate SHALL support arbitrary project-owned validation commands as execution gates. The generic execution layer SHALL own configurable scope/profile participation, dependencies, services, environment handling, timeout, retry, requiredness, logs/artifacts and final blocking composition.

The execution layer SHALL NOT infer application-domain policy from a tool name.

#### Scenario: Project API tests are a shell or package command
- **GIVEN** a required API-test command defined by the project
- **WHEN** the selected profile/component includes that step
- **THEN** Harness-Gate executes the command with its configured services/environment/timeouts
- **AND** a required failed execution blocks the final workflow
- **AND** no API-test-framework-specific Core branch is required

#### Scenario: Project validation requires PostgreSQL
- **GIVEN** a project-owned full-stack smoke step declaring a PostgreSQL service
- **WHEN** Harness-Gate executes the step
- **THEN** it supplies the configured isolated service contract
- **AND** preserves project-defined validation logic outside Harness-Gate

### Requirement: Structured result adapters SHALL enrich diagnostics without acquiring policy authority

Harness-Gate MAY support reusable structured-result formats or adapters, including JUnit, SARIF or versioned declared JSON contracts. Structured result ingestion SHALL preserve provenance and improve machine/human diagnostics, but SHALL NOT independently define Harness-Gate requiredness, thresholds, ratchets, release approval or final generic PASS/FAIL unless those semantics are governed by an explicit accepted Harness-Gate policy contract.

#### Scenario: E2E command emits JUnit
- **GIVEN** a project-owned E2E command emits a valid JUnit report
- **WHEN** a structured result adapter is configured
- **THEN** Harness-Gate may expose test counts, failures and locations in the unified report
- **AND** the execution gate remains governed by configured Harness-Gate requiredness
- **AND** the JUnit producer does not gain release authority

#### Scenario: Structured result is malformed
- **GIVEN** policy/configuration requires a structured result for a required gate
- **WHEN** the result cannot be parsed or validated
- **THEN** Harness-Gate fails closed according to the declared result contract
- **AND** does not silently fall back to a favorable interpretation

### Requirement: Quality collector plugins SHALL remain measurement-only extensions

Measurements intended for generic quality policy SHALL enter through the accepted collector/evidence boundary. Collectors SHALL produce normalized facts with compatible source/config/tool/series identity; the released Rust core SHALL retain requiredness, thresholds, baseline/ratchet/debt semantics, cross-component aggregation and final generic decisions.

#### Scenario: Coverage and complexity feed CRAP policy
- **GIVEN** a certified ecosystem collector produces accepted coverage and complexity evidence
- **WHEN** the corresponding quality policy requires CRAP
- **THEN** the evidence enters the generic Rust evaluator
- **AND** the collector does not decide whether delivery passes

#### Scenario: Ecosystem has no certified CRAP series
- **GIVEN** a project ecosystem whose CRAP measurement series is not certified
- **WHEN** quality verification runs
- **THEN** CRAP remains explicitly unsupported
- **AND** Harness-Gate does not combine unrelated tool outputs into an invented CRAP value

### Requirement: Tool/framework participation SHALL be configuration-driven unless a reusable protocol boundary requires product support

A new application validation tool or framework SHALL NOT require Harness-Gate Generic Core code changes merely to execute its project-owned command. Product-specific support MAY be added for reusable protocol/format/certification boundaries, but SHALL be justified independently from the popularity of a particular tool.

#### Scenario: Unknown future test runner is introduced
- **GIVEN** an executable future test runner unknown when Harness-Gate was released
- **WHEN** its invocation, services, environment and result contract can be represented by existing generic configuration
- **THEN** it participates in required verification without schema or Generic Core redesign

#### Scenario: Reusable result protocol needs native support
- **GIVEN** multiple ecosystems produce the same stable structured result protocol
- **WHEN** native parsing would materially improve diagnostics or trust validation
- **THEN** Harness-Gate may add a protocol adapter
- **AND** that adapter remains generic to the protocol rather than embedding one application framework's test implementation

### Requirement: Dogfood migration SHALL preserve existing project assurance before authority transfer

When Harness-Gate is dogfooded on an existing project with a mature validation workflow, the existing required assurance SHALL be treated as a migration baseline/oracle until parity and fail-closed replacement behavior are evidenced. Harness-Gate SHALL NOT obtain a successful migration result by deleting, weakening, skipping or silently dropping existing valid project gates.

#### Scenario: Existing project has a full-stack smoke gate
- **GIVEN** Arc-Admin currently requires a project-owned full-stack smoke test
- **WHEN** Harness-Gate shadow configuration is introduced
- **THEN** the equivalent validation remains represented and required in the applicable full/CI flow
- **AND** any inability to express it is classified as a Harness-Gate capability gap
- **AND** the Arc-Admin gate is not removed to make parity pass

#### Scenario: Imported execution config contains unsupported semantics
- **GIVEN** an existing execution-plane rule cannot be represented losslessly
- **WHEN** import/migration is attempted
- **THEN** Harness-Gate reports the unsupported mapping explicitly
- **AND** does not silently discard the rule
- **AND** authority transfer remains blocked until the gap is resolved or explicitly reviewed

### Requirement: Compatible execution import SHALL be deterministic and detect declaration loss

The importer SHALL preserve compatible aliases, components, scope, services,
parsers, profiles, required steps, dependencies, environment declarations and
timeouts without resolving host environment values or executing project commands.
It SHALL reject unsupported versions/fields, invalid references and lossy
deserialization before publishing output. It SHALL preserve every selected-step
blocker, including steps outside the explicit required-step list. It SHALL NOT
overwrite source or existing output files.

#### Scenario: Frozen Arc-Admin configuration is imported on different hosts
- **GIVEN** identical frozen Arc-Admin v2 source bytes and different host overrides
- **WHEN** execution-only import runs into unused destinations
- **THEN** generated TOML and parity reports are byte-identical
- **AND** all 25 selected-step blockers and 23 explicit required IDs are retained
- **AND** Arc's secret-policy default and command working-tree input are explicit

#### Scenario: Unknown blocker or coerced value is present
- **GIVEN** source contains an unsupported field, interpolation-sensitive literal, invalid dependency or a set entry that would be dropped
- **WHEN** execution-only import runs
- **THEN** it fails visibly with a diagnostic and publishes neither output file
- **AND** the existing source remains unchanged

#### Scenario: Runtime migration has unresolved semantics
- **GIVEN** Arc global environment overrides and runtime/prelude/CI parity are unresolved
- **WHEN** import is attempted without an explicit execution-only scope
- **THEN** full migration fails before writing files and lists the blockers
- **AND** execution-only output, if explicitly requested, still records blocked authority transfer
- **AND** no quality configuration, CI profile or replacement authority is invented

#### Scenario: Offline import encounters a static resource conflict
- **GIVEN** supported execution declarations contain a duplicate log destination
- **WHEN** execution-only import runs
- **THEN** it uses the production loader's static semantic/resource validation and rejects the conflict before publishing either output
- **AND** host environment values do not influence validation or imported bytes
- **AND** valid serial configurations remain compatible without invented dependencies

### Requirement: Import SHALL expose migration effort and configuration duplication metrics

The deterministic import report SHALL count import operations, re-entered steps,
manual execution-config edits, preserved source scalar values, configuration
bytes and duplicated step definitions. Unmeasured human elapsed time and runtime
integration effort SHALL remain explicitly unmeasured rather than reported as zero.

#### Scenario: Arc-Admin retains its original configuration during import review
- **GIVEN** Arc-Admin's existing 25-step configuration remains required
- **WHEN** the compatible configuration is generated by one import command
- **THEN** the report records zero re-entered steps and manual execution-config edits
- **AND** one additional configuration duplicates all 25 step definitions
- **AND** the report does not claim measured adoption time or completed runtime integration

### Requirement: Arc-Admin quality configuration SHALL preserve accepted measurement and trust contracts

Arc-Admin quality configuration SHALL bind the Angular frontend, Rust backend and frontend/backend API relationship using existing generic quality packs. It SHALL preserve accepted Rust coverage/risk/CRAP series, thresholds, lineage, debt, ratchet and fail-closed semantics, and require a trusted baseline provider. Angular/TypeScript CRAP SHALL remain explicitly unsupported unless separately certified. Project-owned E2E, API, smoke, generation and deployment commands SHALL remain execution hooks.

#### Scenario: Configure quality beside the pinned execution import
- **GIVEN** the imported Arc-Admin flow declares `full` and `hook`
- **WHEN** the separate quality configuration is validated with that flow
- **THEN** component bindings and the frontend-to-backend API relationship resolve
- **AND** `full` includes required Rust coverage/CRAP and relationship policies while `hook` declares partial assurance
- **AND** Angular CRAP is an unsupported diagnostic without a numeric substitute
- **AND** the existing 25 execution steps remain unchanged and no `ci` profile is invented

#### Scenario: Required Rust binding or baseline loses compatibility
- **GIVEN** the accepted certified Rust series and required Git merge-base provider
- **WHEN** the configured series changes incompatibly, a required policy is omitted or the required provider is removed
- **THEN** configuration validation fails
- **AND** runtime baseline evidence remains subject to the existing trusted provenance and lineage checks
- **AND** reference pack metadata alone does not establish an Arc-Admin quality PASS

### Requirement: Shadow observations SHALL distinguish runtime evidence from blocked comparisons

Shadow comparison SHALL retain equivalent source identities, unchanged blocker configuration, exact commands and exits, every selected traditional gate including blockers outside the explicit required list, service/environment observation limits, diagnostics and hashed reports/artifacts. Each discrepancy SHALL have exactly one category: Arc-Admin issue, Harness-Gate capability gap, Harness-Gate UX gap, or expected stricter generic-quality difference. Validation PASS/FAIL/ERROR SHALL NOT imply workflow/lifecycle state or merge-authority transfer.

#### Scenario: Complete production loading rejects the structurally imported flow
- **GIVEN** both engines are invoked against the same pinned tracked source and all Arc-Admin blockers remain unchanged
- **WHEN** Arc-Admin completes its full profile but Harness-Gate rejects shared-service ordering before scope or dispatch
- **THEN** every traditional Harness-Gate gate is recorded as NOT_RUN and selection as NOT_COMPUTED with a classified explanation
- **AND** PostgreSQL and environment runtime parity remain unestablished rather than inferred from preserved declarations
- **AND** pre-existing Arc-Admin reports are never attributed to Harness-Gate
- **AND** the complete classified observation does not claim successful parity or authorize lifecycle advancement

### Requirement: CI cost and authority design SHALL preserve assurance and distinguish unmeasured work

The Arc-Admin cost record SHALL distinguish before, observed shadow and proposed target topology, retain source/run identities and timestamp-based wall/runner evidence, and inventory duplicate command execution separately from duplicate authoritative measurement production. Unmeasured shadow or target cost SHALL remain unknown. Each selected project command and supported authoritative measurement identity SHALL have a single proposed owner; evidence consumers SHALL reuse compatible authenticated artifacts rather than recollect equivalent authoritative facts. Project-owned test implementations and all existing blocking obligations SHALL remain intact until a separately accepted replacement proves runtime parity and fail-closed behavior.

#### Scenario: Only the before CI run and blocked local shadow receipts exist
- **GIVEN** retained self-hosted before job timestamps and local Harness-Gate pre-dispatch failures without elapsed timings
- **WHEN** the cost ledger is reproduced
- **THEN** before wall and runner occupancy are derived from the retained CI timestamps
- **AND** local native step durations are explicitly distinguished from CI occupancy
- **AND** added shadow cost and target savings remain unmeasured, with task 7.1 pending

#### Scenario: Proposed topology removes duplicate ownership without weakening gates
- **GIVEN** all 25 project hooks, prelude and CI-only checks plus required generic-quality policies
- **WHEN** a single-execution and single-measurement-producer topology is designed
- **THEN** project commands, requiredness, scope/profile semantics, service isolation and baseline/ratchet contracts remain obligations
- **AND** structured results and aggregate jobs consume existing evidence without taking measurement authority or recollecting equivalent facts
- **AND** Angular CRAP remains unsupported without a numeric producer
- **AND** existing CI/arc-flow gates remain until replacement ownership, runtime parity and fail-closed evidence are accepted separately


### Requirement: Guaranteed serial execution SHALL preserve shared-service compatibility

For consumers of the same service identity, the production loader SHALL recognize guaranteed single-worker dispatch as ordering, as specified in ADR-0050. It SHALL NOT insert dependencies or alter project scope, profiles, commands or requiredness to obtain compatibility. A configuration that permits multiple workers SHALL retain the dependency-based shared-service conflict check. Duplicate logs and existing injection safeguards SHALL remain enforced.

#### Scenario: Serial consumers execute without inferred dependencies
- **GIVEN** two project steps share a service and have no dependency path
- **WHEN** execution is explicitly serial or has only one worker
- **THEN** production loading succeeds and selected consumers execute one at a time
- **AND** no dependency is added and no unselected step is pulled into scope
- **WHEN** the same configuration enables multiple workers
- **THEN** production loading rejects unordered sharing before execution


#### Scenario: Bounded self-hosted workloads complete with a retained quality failure
- **GIVEN** paired before/shadow workloads run on a self-hosted runner at pinned source and tool revisions
- **WHEN** all traditional steps pass but trusted quality workflow inputs are absent
- **THEN** the actual workload segment and job occupancy costs are retained with failed quality exit codes
- **AND** full-quality cost, original-topology savings and authority transfer remain unestablished
- **AND** the missing host provisioning is tracked for integration remediation rather than fabricated or waived

### Requirement: Early quality failures SHALL identify the requested profile

Quality preparation failures SHALL retain the requested profile in machine and human diagnostics. They SHALL remain blocked in the configuration phase without inventing policy participation, native measurements, a project quality report or baseline acceptance.

#### Scenario: Trusted workflow state or keys are unavailable
- **GIVEN** a selected quality workflow requires trusted state and keys
- **WHEN** either input is missing during preparation
- **THEN** verification fails with the requested profile and the concrete input error
- **AND** overall and full-quality outcomes remain blocked with no project quality report
- **AND** static configuration success is not presented as successful runtime quality collection
