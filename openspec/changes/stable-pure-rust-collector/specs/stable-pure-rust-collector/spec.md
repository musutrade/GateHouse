# Stable pure-Rust collector

## ADDED Requirements

### Requirement: Rust-only installed implementation

The first-party Rust collector MUST use Rust for all its installed implementation
and MUST NOT require Python, rustc-dev, rustc_private, RUSTC_BOOTSTRAP or -Z flags.

#### Scenario: Clean supported host

- GIVEN only the documented external dependencies are installed
- WHEN the verified precompiled collector is installed and run
- THEN it collects supported measurements through stable interfaces without a private interpreter or compiler environment.

### Requirement: No implicit measurement migration

New source/coverage measurements MUST use a distinct series and reviewed boundaries.
Core policy authority, requiredness, trust, thresholds and historical baselines MUST
remain intact. Unsupported or ambiguous measurement MUST NOT become success.

#### Scenario: Incompatible historical evidence

- GIVEN historical MIR evidence and a new stable-source measurement
- WHEN a comparison lacks a reviewed compatible transition
- THEN Core rejects the comparison and no baseline is automatically rewritten.

### Requirement: Stable required CI and explicit release transition

Required CI MUST NOT build/run a compiler-private backend. New legacy publication
MUST fail until the pure-Rust stable replacement has reviewed actual acceptance.

#### Scenario: Legacy publication request during migration

- GIVEN a signing, publication or installer packet for the old workflow
- WHEN the workflow starts
- THEN it rejects the request before release environment approval and publication side effects.

### Requirement: Evidence-backed macro support and focused source fixes

Source availability alone MUST NOT certify an accurate generated-code metric.
Macro presence alone MUST NOT justify blanket unsupported classification. A concrete
defect MUST have a minimal reproducer, identified responsible layer and tracked focused
fix or capability request. Fixed source/version and real regression evidence MUST be
verified before promoting support; upstream PR submission alone is insufficient.

#### Scenario: Same macro implementation, different invocation

- GIVEN the same dependency source used with different invocation inputs or build configuration
- WHEN generated-code complexity or coverage is collected
- THEN the collector binds each result to its actual generated owners/configuration and execution evidence rather than assigning a value from the macro implementation source.

#### Scenario: Mapping defect in an available dependency

- GIVEN a reproducible source/coverage ownership defect
- WHEN a source-level dependency patch is proposed
- THEN a focused PR and regression fixture track the fix, and support is promoted only after the fixed dependency version and actual measurement outcomes are validated.
