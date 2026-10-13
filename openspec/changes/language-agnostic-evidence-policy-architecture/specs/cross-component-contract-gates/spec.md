# Capability: Cross-Component Contract Gates

## ADDED Requirements

### Requirement: Evaluate explicit producer-consumer contract relationships
Harness-Gate SHALL evaluate cross-component contract gates from explicit project relationships rather than guessing producer/consumer links from repository layout or file names.

#### Scenario: Angular frontend consumes Rust OpenAPI contract
- **GIVEN** a project model linking an Angular frontend consumer to a Rust API producer through an OpenAPI contract
- **WHEN** the contract gate runs
- **THEN** the evaluated producer, consumer, and contract identities are explicit in the result
- **AND** the relationship is not inferred from directory names.

### Requirement: Detect incompatible contract changes independently of local component tests
A cross-component gate SHALL be able to fail when a shared contract change is incompatible even if each individual component's local tests pass.

#### Scenario: Backend renames a response property
- **GIVEN** the backend changes a required response field from `email` to `primaryEmail`
- **AND** backend and frontend local test suites both pass
- **WHEN** compatibility is evaluated against the accepted contract baseline or consumer expectation
- **THEN** the cross-component gate reports the breaking change
- **AND** the release aggregate cannot treat the local test passes as sufficient contract evidence.

### Requirement: Retain contract provenance and generated-client evidence
Contract evidence SHALL record the source contract identity/digest, producer and consumer components, tool/rule versions, base/head identity when comparing changes, and generated-client artifacts or digests when generated clients are in policy scope.

#### Scenario: Generated client is stale
- **GIVEN** an OpenAPI contract changes but the checked-in generated frontend client still corresponds to the previous contract digest
- **WHEN** generated-client drift is required
- **THEN** the gate reports stale generated-client evidence
- **AND** identifies both the contract digest and generated artifact digest involved.

### Requirement: Treat missing required cross-component evidence as fail closed
If a required contract relationship cannot be evaluated because its contract, baseline, consumer evidence, generator evidence, or compatible tool series is missing, the gate SHALL return a blocking measurement/contract error rather than pass.

#### Scenario: Contract baseline is unavailable
- **GIVEN** a policy requires breaking-change comparison
- **AND** the required base contract object cannot be resolved
- **WHEN** evaluation runs
- **THEN** the result is blocking and explicitly identifies the missing baseline
- **AND** no compatibility claim is made.

### Requirement: Use generic typed contract metrics and explicit scope
Relationship-scoped rules SHALL select provider-owned `contract/v1` subjects and
compare `contract.schema_valid`, `contract.breaking_changes`,
`contract.client_drift` and `contract.compatible` using the generic typed policy
engine. Required baseline and consumer/client provenance SHALL be checked before
supported metrics are compared. Missing capabilities SHALL retain their existing
unavailable state without fabricated values.

#### Scenario: Detect contradictory generated-client drift
- **GIVEN** retained contract and client artifacts and the input contract digest used by the generator
- **WHEN** a reported drift boolean disagrees with digest equality
- **THEN** the contract gate returns a blocking measurement error
- **AND** the report retains both artifact digests and participant identities.
