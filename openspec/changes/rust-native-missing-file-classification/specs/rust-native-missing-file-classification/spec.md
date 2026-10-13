## ADDED Requirements

### Requirement: File classification SHALL depend on complete authenticated mapping
The classifier SHALL retain every production source hash and bind conclusions to
compiler, target, cfg, features, sample selection, complete definitions and LLVM
ownership. Missing or tampered evidence SHALL remain measurement_error.

#### Scenario: Missing export or owner
- **WHEN** any declared executable owner lacks its LLVM counters
- **THEN** classification SHALL reject absence inferences and report measurement_error.

### Requirement: Non-applicability SHALL require executable-source evidence
Only compiler-proven absence of runtime code in the declared compilation SHALL
permit not_applicable. Module forwarding SHALL NOT imply parent machine code;
macro syntax or zero AST functions SHALL NOT prove runtime absence. Files outside
selected cfg SHALL retain explicit scoped selection evidence or measurement_error.

#### Scenario: Macro generates constants and runtime methods
- **WHEN** expansion yields CTFE definitions and executable methods
- **THEN** the methods SHALL retain independent real counters and the file SHALL NOT be not_applicable.

#### Scenario: Declaration and unexecuted function
- **WHEN** a complete compilation observes declarations only in one file and zero-count runtime owners in another
- **THEN** only the first SHALL be not_applicable and the second SHALL retain positive denominators and real zero hits.

#### Scenario: Conditional module or omitted export
- **WHEN** a source is not compiler-loaded
- **THEN** a complete alternate-feature compilation of identical sources, tools, target, Cargo inputs and test selection MAY prove feature-scoped exclusion; without this witness absence SHALL remain measurement_error.

### Requirement: Reports SHALL preserve series and acceptance boundaries
Classification SHALL NOT delete inventory entries, fabricate 0/0, change thresholds,
accept baselines or substitute MIR observations for historical LLVM file summaries.

#### Scenario: Archived backend and independent fixture capture
- **WHEN** historical binaries are unavailable but reviewed raw mapping remains
- **THEN** reports SHALL distinguish retained mapping inspection from new native certification and preserve unauthenticated historical LLVM gaps.
