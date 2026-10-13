# Capability: Normalized Quality Evidence

## ADDED Requirements

### Requirement: Introduce a versioned generic evidence envelope
Harness-Gate SHALL define a versioned normalized evidence envelope, initially `harness-evidence/v1`, that records project/component identity, collector identity, measurement series, subject identity, metrics, capabilities, commit/target/run provenance, raw artifact references, source integrity, and measurement status.

#### Scenario: Normalize equivalent metrics from different ecosystems
- **GIVEN** a Rust collector and a TypeScript collector that both report cyclomatic complexity
- **WHEN** their outputs are normalized
- **THEN** both use the same generic evidence envelope
- **AND** their collector, series, language, source, and raw artifact provenance remain distinct.

### Requirement: Preserve raw evidence and existing Rust contracts
The generic envelope SHALL coexist with existing tool-specific and Rust-specific evidence contracts. Existing accepted evidence SHALL NOT be silently rewritten or broadened in place merely to fit the generic model.

#### Scenario: Project current Rust complexity evidence
- **GIVEN** a valid record conforming to the existing Rust `quality-evidence.schema.json`
- **WHEN** it is projected into `harness-evidence/v1`
- **THEN** the original record remains retained and addressable
- **AND** the normalized record preserves its series identity, source digest, symbol identity, raw counts, and metric value.

### Requirement: Distinguish capability and measurement states
Normalized evidence SHALL distinguish at least `supported`, `unsupported`, `not_configured`, `not_collected`, `measurement_error`, and `not_applicable` capability/measurement states. Non-numeric states MUST NOT be coerced into numeric pass values.

#### Scenario: Unsupported branch coverage
- **GIVEN** a collector whose measurement contract does not support branch coverage
- **WHEN** normalized evidence is produced
- **THEN** branch coverage is marked `unsupported` with provenance
- **AND** no `0%` or `100%` branch coverage value is fabricated.

### Requirement: Validate provenance and integrity fail closed
Harness-Gate SHALL reject normalized evidence whose required commit/base/run identity, source digest, subject identity, raw artifact reference, or series metadata is missing, stale, malformed, duplicated, or incompatible for the requested evaluation.

#### Scenario: Detect stale evidence
- **GIVEN** normalized evidence whose source digest does not match the evaluated source
- **WHEN** the evidence is validated
- **THEN** validation returns `measurement_error`
- **AND** the evidence cannot satisfy a required gate.

### Requirement: Keep incompatible measurement series incomparable
Measurements from incompatible series SHALL NOT be compared numerically for regression or baseline acceptance unless a separately defined migration explicitly establishes compatibility.

#### Scenario: Analyzer rule version changes
- **GIVEN** base evidence produced by complexity rule version 1 and head evidence produced by rule version 2
- **WHEN** a ratchet attempts to compare them
- **THEN** the comparison is rejected as incompatible
- **AND** the system does not claim improvement or regression from the numeric values alone.

### Requirement: Use explicit typed values and deterministic serialization
The v1 envelope SHALL use discriminated ratio, count, boolean, duration, size and
exact decimal forms, plus exact rational CRAP values preserving native Rust
numerator/denominator integers without rounding. Ratios and rationals SHALL use
positive denominators. Decimal and rational CRAP SHALL remain distinct series
value types. Metric names SHALL match their declared series value types. Canonical
serialization SHALL validate the complete batch and provenance before emitting
UTF-8 JSON with sorted object keys, compact separators and preserved array order.

#### Scenario: Reject ambiguous or fabricated values
- **GIVEN** a metric with an untyped JSON number, unknown type, or zero denominator
- **WHEN** the normalized record is validated
- **THEN** validation returns `measurement_error`.
- **AND** every non-supported capability is forbidden from carrying any metric value.

### Requirement: Enforce explicit capability availability policy
Capability requirements SHALL specify component, metric, required/informational
mode and a blocked/measurement_error outcome for unavailable required metrics.
Availability results SHALL preserve the original capability state and evidence ID
and SHALL NOT constitute a numeric threshold or release decision.

#### Scenario: Preserve informational unsupported state
- **GIVEN** valid unsupported evidence with raw provenance
- **WHEN** an informational capability requirement is evaluated
- **THEN** the result remains `unsupported` without a numeric value.
- **AND** a required requirement produces its explicit blocking outcome.

#### Scenario: Reject a false support claim
- **GIVEN** a supported capability with missing metric evidence
- **WHEN** either required or informational availability is evaluated
- **THEN** validation returns `measurement_error` before availability evaluation.
