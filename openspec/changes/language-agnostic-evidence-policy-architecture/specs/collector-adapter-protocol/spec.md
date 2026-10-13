# Capability: Collector Adapter Protocol

## ADDED Requirements

### Requirement: Separate collection from policy evaluation
Collectors and adapters SHALL measure and normalize facts but SHALL NOT be authoritative for final delivery pass/fail decisions. Final policy evaluation belongs to Harness-Gate.

#### Scenario: Collector reports a low coverage value
- **GIVEN** a collector that measures line coverage as 0.72
- **WHEN** it emits evidence
- **THEN** the collector reports the measurement and provenance
- **AND** does not decide whether 0.72 is acceptable for release.

### Requirement: Declare collector capabilities and measurement contracts
Each collector SHALL declare its supported metric capabilities, collector/tool identity and version, rule or parser version where applicable, required inputs, platform/toolchain constraints, and output series identity.

#### Scenario: Collector lacks branch instrumentation
- **GIVEN** a coverage collector that supports line/function/region coverage but not branch coverage
- **WHEN** its capabilities are inspected
- **THEN** branch coverage is explicitly declared unsupported
- **AND** consumers do not infer support from the presence of other coverage metrics.

### Requirement: Produce normalized evidence or explicit measurement failure
A collector invocation SHALL either produce schema-valid normalized evidence linked to raw artifacts or produce a structured collection/measurement error. Silent omission of requested required metrics is not allowed.

#### Scenario: Required analyzer cannot parse source
- **GIVEN** production source outside the collector's validated syntax subset
- **WHEN** collection is requested for that source
- **THEN** the collector returns a structured measurement error
- **AND** the required policy path fails closed instead of treating the source as clean.

### Requirement: Reuse retained evidence when projection is sufficient
Adapters SHALL be able to normalize already-retained evidence without rerunning expensive tests or instrumentation when the existing evidence contains the required facts and compatible provenance.

#### Scenario: Shadow generic evaluation of current Rust CI
- **GIVEN** `ci_quality.py collect` has already retained compatible Rust coverage/risk/traceability artifacts for a run
- **WHEN** the generic shadow evaluator needs normalized evidence
- **THEN** an adapter projects those retained artifacts
- **AND** does not rerun the expensive Rust collection solely for normalization.

### Requirement: Isolate collector implementation from release runtime where practical
Development/CI-only collectors MAY be implemented outside the release binary, but their identity, version, invocation contract, artifacts, and failures SHALL remain reproducible and reviewable.

#### Scenario: Python quality collector remains development-only
- **GIVEN** a Python-based collector used only in CI
- **WHEN** Harness-Gate records its evidence
- **THEN** its version and invocation are captured
- **AND** the collector does not need to be linked into the shipped Harness-Gate binary.

### Requirement: Bind invocation to a versioned request and exclusive response
The v1 request SHALL contain project/component IDs, expected collector identity,
commit/base/target/run context, requested capabilities, canonical workspace/output
roots and collection parameters. The v1 response SHALL contain only its schema
version, normalized evidence, declared raw artifacts and a typed error. Evidence
and error outcomes SHALL be mutually exclusive; final release decisions SHALL be
rejected. Internal and subprocess adapters SHALL use the same validation boundary.

#### Scenario: Consume either adapter without transport-dependent policy
- **GIVEN** equivalent synthetic retained evidence from an internal callable and an external stdin/stdout adapter
- **WHEN** the runner validates each response and the capability consumer evaluates the returned records
- **THEN** normalized records and capability outcomes are identical.
- **AND** unsupported and measurement-error states retain their semantics.

### Requirement: Reject invalid collection before policy consumption
The runner SHALL return typed fail-closed errors for nonzero subprocess exit,
timeout, malformed JSON, output-root escape, undeclared or missing artifacts,
stale provenance, duplicate subject/series pairs and artifact tampering. Every
requested capability SHALL have an explicit state in each returned record.

#### Scenario: Collector exits unsuccessfully after printing evidence
- **GIVEN** a subprocess that prints a schema-valid response but exits nonzero
- **WHEN** collection completes
- **THEN** the runner returns `subprocess_exit` without usable evidence.

#### Scenario: Tampered or undeclared raw output
- **GIVEN** a dedicated empty output root and a collector response
- **WHEN** a raw file is unregistered, escapes through a path or symlink, or differs from its declared digest/size
- **THEN** the runner returns the corresponding typed artifact failure before policy evaluation.

#### Scenario: Stale or duplicate evidence
- **GIVEN** evidence from another commit/base/target/run or repeated subject/series identity
- **WHEN** either adapter submits the batch
- **THEN** the runner returns `stale_context` or `duplicate_subject` without usable evidence.

#### Scenario: Timeout with inherited output handles
- **GIVEN** a POSIX subprocess and its child retain stdout beyond the deadline
- **WHEN** the runner times out
- **THEN** it kills the process group and returns `timeout`.
- **AND** this process cleanup does not claim an operating-system sandbox.

### Requirement: Retain same-run advisory shadow evidence
Generic shadow CI SHALL consume the existing Rust collection artifact from the
same workflow run and attempt, verify the original base/head/run identity, and
retain projection and compatibility reports on success or failure. It SHALL NOT
repeat Rust test, coverage, risk or matrix collection.

#### Scenario: Missing or partial current-run collection
- **GIVEN** the current Rust collection fails or its artifact is missing
- **WHEN** the advisory shadow job runs
- **THEN** it reports a comparison or measurement error where execution is available
- **AND** it does not substitute another run or manufacture successful acceptance.

#### Scenario: Required authority remains unchanged
- **GIVEN** the generic shadow job disagrees with the current Rust result
- **WHEN** CI evaluates release eligibility
- **THEN** `Required Quality Aggregate` retains its existing name, dependencies and fail-closed behavior
- **AND** Rust remains the sole required authority while the disagreement blocks equivalence acceptance.
