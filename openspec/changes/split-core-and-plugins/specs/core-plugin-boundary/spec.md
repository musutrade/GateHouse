## ADDED Requirements

### Requirement: Host and plugin responsibilities are separated
The core (GateHouse) SHALL own protocol validation, execution budgets, result
decision, thresholds and baseline comparison. Plugins (GateProbes) SHALL own
language/tool-specific measurement and return standard results. A plugin SHALL
NOT decide gate pass/fail and SHALL NOT approve permission.

#### Scenario: A collector returns a measurement
- **GIVEN** a plugin that measures Rust, TypeScript/Angular or HTTP/JSON-contract facts
- **WHEN** it returns a standard result to the core
- **THEN** the core alone decides the gate result, threshold and baseline outcome
- **AND** the plugin cannot set requiredness, pass/fail or permission.

#### Scenario: A plugin attempts to self-approve
- **GIVEN** a plugin result that asserts a gate passes or permission is granted
- **WHEN** the core evaluates the result
- **THEN** the assertion is ignored and the core's own decision stands.

### Requirement: Cut dependencies before moving repositories
The split SHALL remove core→plugin source coupling, assign one schema source per
role, and separate fixture ownership BEFORE any repository move. Moving
repositories SHALL NOT be the first step.

#### Scenario: Core references a plugin fixture
- **GIVEN** core source containing an `include_str!` that reaches a plugin fixture path
- **WHEN** the dependency cut is performed
- **THEN** the decision-engine fixture is moved into core
- **AND** core builds with no reference into plugin fixture paths.

#### Scenario: Schemas with duplicate basenames
- **GIVEN** the same schema basename present under the core and plugin schema roots
- **WHEN** the source of truth is assigned
- **THEN** exactly one role owns the editable source (host contract / generic evidence / plugin raw output)
- **AND** any other copy is marked generated and is not edited independently.

### Requirement: Core builds and tests without plugin source
The core SHALL build and test without checking out or compiling plugin source.

#### Scenario: Plugin source is absent
- **GIVEN** a checkout of the core repository only
- **WHEN** the core is built and its tests run
- **THEN** the build and tests succeed without any plugin source present.

### Requirement: Core CI consumes pinned plugin artifacts
Core CI SHALL consume pinned plugin release artifacts and SHALL NOT build the
latest plugin source on each run. The plugin repository SHALL test, package and
publish on its own.

#### Scenario: A pinned collector version is used
- **GIVEN** core CI configured as in this change
- **WHEN** the quality collection runs
- **THEN** it uses an explicitly pinned plugin artifact version
- **AND** it does not compile collector source.

#### Scenario: An incompatible collector version is supplied
- **GIVEN** a collector artifact whose protocol version is incompatible with the core
- **WHEN** core CI starts the collection
- **THEN** the run fails closed with an explicit version diagnostic.

### Requirement: Independent release and rollback
A plugin version upgrade SHALL NOT require re-releasing or re-compiling the core.
The core and plugins SHALL be able to release and roll back independently.

#### Scenario: A plugin is upgraded
- **GIVEN** a newly released plugin version
- **WHEN** the next run selects that version
- **THEN** no core rebuild or re-release is required
- **AND** the previous plugin version can be selected again for rollback.

### Requirement: Preserve the existing CLI and package identity
Only repository names SHALL change to GateHouse / GateProbes. The
`harness-gate` command, crate, published package and legacy install entry SHALL
remain compatible during the split.

#### Scenario: An existing user upgrades
- **GIVEN** an installation created through the legacy `harness-gate` entry
- **WHEN** the split proceeds
- **THEN** the existing command and install entry continue to work.

### Requirement: Re-verify signed provenance after the move
When a repository move changes signing identity, the plugin release chain SHALL
re-verify Sigstore certificate identity and provenance for the new
repository/workflow, not only the download URL.

#### Scenario: Plugin release after the move
- **GIVEN** the plugin repository after the move
- **WHEN** a plugin release is produced
- **THEN** its attestation and certificate identity reference the new repository/workflow
- **AND** verification fails if the identity does not match.

### Requirement: No new plugin platform or hot-plugging
The split SHALL NOT introduce a plugin marketplace, dynamic discovery, a
resident management service, hot-plugging, or a required host restart. Plugins
remain subprocess-based; a new version is selected on the next run.

#### Scenario: Selecting a new plugin version
- **GIVEN** a running host that uses a plugin as a subprocess
- **WHEN** a different plugin version is selected
- **THEN** the next invocation uses the new version
- **AND** no host restart or hot-plug mechanism is required.

### Requirement: No measurement or authority change as a side effect
The split SHALL NOT change gate names, branch protection, release authority,
accepted contracts, or any measurement semantic (CRAP, coverage, complexity,
contract, critical-path).

#### Scenario: Split completes
- **GIVEN** the split has been executed
- **WHEN** existing required checks and release authority are inspected
- **THEN** they are unchanged
- **AND** measurement semantics match the pre-split accepted behavior.
