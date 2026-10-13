## ADDED Requirements

### Requirement: Keep generic delivery semantics authoritative in Rust
Harness-Gate SHALL provide its generic evidence, project, policy, ratchet, relationship, and project-report decision semantics from the released Rust core after migration acceptance. External collectors MAY be implemented in any language but SHALL NOT own final delivery decisions.

#### Scenario: External TypeScript collector feeds Rust decisions
- **GIVEN** a valid TypeScript/Angular collector response and normalized `harness-evidence/v1` records
- **WHEN** generic policy and project aggregation are evaluated after authority transfer
- **THEN** the Rust core produces the authoritative gate and project result
- **AND** the collector does not supply or override requiredness, pass/fail, debt, or aggregate decisions.

### Requirement: Preserve accepted machine contracts during consolidation
The migration SHALL preserve accepted project, evidence, policy, result, and project-report machine contracts unless a separately reviewed spec delta explicitly changes them. Implementation convenience SHALL NOT silently narrow valid input or widen invalid input.

#### Scenario: Existing retained evidence is replayed
- **GIVEN** retained Rust or TypeScript/Angular evidence accepted under the current contracts
- **WHEN** the Rust candidate parses, validates, evaluates, and reports it
- **THEN** accepted contract fields and machine semantics remain compatible with the established reference behavior.

### Requirement: Prove semantic equivalence before authority transfer
The Rust candidate SHALL run in non-authoritative differential mode against the established Python reference semantics using the same retained project, policy, evidence, base, selection, source, artifact, and expected context. Any unexplained semantic mismatch SHALL block authority transfer.

#### Scenario: Differential replay finds a mismatch
- **GIVEN** byte-identical accepted inputs supplied to Python reference and Rust candidate evaluators
- **WHEN** canonical machine outputs disagree on a contractually significant field
- **THEN** the migration is blocked
- **AND** the mismatch is retained with field-level evidence until resolved or covered by a separately reviewed contract delta.

#### Scenario: Both real ecosystems are equivalent
- **GIVEN** the retained Rust corpus and retained TypeScript/Angular corpus including positive and negative cases
- **WHEN** differential acceptance completes
- **THEN** there are zero unexplained mismatches before Rust generic semantics become authoritative.

### Requirement: Preserve fail-closed capability and measurement semantics
Rust consolidation SHALL preserve distinct supported, unsupported, not-applicable, not-configured, not-collected, and measurement-error behavior, typed value comparison, measurement-series compatibility, and required gate aggregation. Missing or invalid data SHALL NOT become a favorable numeric or delivery result.

#### Scenario: Required unsupported capability is evaluated
- **GIVEN** a blocking policy requiring a capability whose evidence state is unsupported
- **WHEN** Rust evaluates the gate
- **THEN** the capability remains explicitly unavailable
- **AND** the required aggregate cannot pass because of a fabricated numeric default.

#### Scenario: Measurement series changes incompatibly
- **GIVEN** base and head evidence whose measurement-series semantics are incompatible
- **WHEN** an incremental or ratchet policy attempts comparison
- **THEN** Rust fails closed without silently reusing the prior baseline or losing historical debt.

### Requirement: Preserve baseline, lineage, debt, and exception semantics
Rust SHALL reproduce accepted baseline compatibility, subject lineage, rename/move handling, debt classification, trend, and exception-review semantics before those Python semantics are demoted.

#### Scenario: Legacy debt improves without clearing threshold
- **GIVEN** compatible base/head evidence for a subject with existing debt
- **WHEN** the head improves but remains below the absolute target
- **THEN** Rust reports the same debt/trend and gate outcome as the established reference behavior.

#### Scenario: Exception metadata is invalid
- **GIVEN** malformed, expired, or otherwise invalid exception metadata
- **WHEN** Rust performs exception review
- **THEN** the error cannot convert a failing result into a pass
- **AND** the aggregate remains fail-closed according to the accepted contract.

### Requirement: Preserve cross-component and project-report semantics
Rust SHALL own generic provider/consumer relationship validation and project/component/local/cross-component aggregation after migration. Project reports SHALL retain the accepted provenance links and indexes needed to identify blocking gates.

#### Scenario: Local gates pass but contract breaks
- **GIVEN** passing component-local frontend and backend gates and a breaking retained provider/consumer contract result
- **WHEN** Rust aggregates the project
- **THEN** the project fails
- **AND** the report identifies the relationship, participating components, blocking policy, and raw evidence provenance.

### Requirement: Keep non-core Python roles intentionally supported
The migration SHALL NOT require CI/dev tooling or ecosystem adapters to be rewritten in Rust merely for language uniformity. A reviewed inventory SHALL classify Python quality modules and SHALL identify which generic-semantic modules are removed, wrapped, or frozen as non-authoritative after transfer.

#### Scenario: Python ecosystem adapter remains in use
- **GIVEN** a Python-based adapter that conforms to the versioned collector protocol
- **WHEN** the Rust core evaluates its normalized evidence
- **THEN** the adapter remains a supported implementation choice
- **AND** no Python generic-semantic module is required to make the authoritative final decision.

#### Scenario: Frozen Python reference is retained after transfer
- **GIVEN** a C-class Python implementation retained for replay, adapter preflight or rollback investigation
- **WHEN** it is invoked after Rust authority transfer
- **THEN** it has no generic release-approval path and the Python decision CLIs require explicit reference-only use
- **AND** reviewed source hashes freeze its generic semantics, with any repair requiring compatibility evidence.

#### Scenario: Migration tooling is retired
- **GIVEN** retained C/D reference tooling and accepted compatibility evidence
- **WHEN** retirement is proposed
- **THEN** live callers have reviewed replacements, equivalent positive/negative coverage is retained, and rollback no longer requires the implementation
- **AND** historical source identities, corpus, decisions and baselines are preserved.

### Requirement: Preserve existing release authority until explicit transfer
The existence of a Rust candidate SHALL NOT by itself change existing required-check identity, branch protection, or release authority. Transfer SHALL occur only after real Rust and TypeScript/Angular corpus equivalence, negative-matrix acceptance, hosted CI success, and documented rollback.

#### Scenario: Rust shadow implementation is incomplete
- **GIVEN** a partial Rust generic-core implementation or unresolved differential mismatch
- **WHEN** required CI evaluates a change
- **THEN** existing release authority remains unchanged
- **AND** the shadow candidate cannot weaken or replace the current required result.
