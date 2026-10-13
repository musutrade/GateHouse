## ADDED Requirements

### Requirement: Production inventory SHALL retain compiler provenance
The analyzer SHALL retain source hashes, macro invocation and expansion origins,
cfg/features, toolchain and compiler arguments for all production executable code.
Zero source AST functions SHALL NOT prove absence of generated executable code.

#### Scenario: Expression-shaped or generated macro
- **WHEN** metrics, anyhow, try_join, task_local, macro_rules or a derive is present
- **THEN** explicit grammar and compiler expansion provenance are required
- **AND** unknown or incomplete ownership remains a measurement error

### Requirement: LLVM production ownership SHALL be complete and unique
Every observed LLVM function instance and region SHALL have verifiable owners.
Closures and async bodies SHALL require their own observations. One-to-many origins
and excluded test mappings SHALL be retained; duplicates and ambiguity SHALL fail.

#### Scenario: Missing closure or duplicate region
- **WHEN** a closure lacks compiler evidence or a region is counted twice
- **THEN** certification fails without borrowing its parent function count

### Requirement: Production metrics SHALL use an independent native series
Raw native counts SHALL determine production line, region and function coverage
and exact rational CRAP. Toolchain, selection, mapping and rules SHALL identify the
series; filtered production SHALL NOT reuse llvm-file-summary-unfiltered/1.

#### Scenario: Incompatible historical measurement
- **WHEN** a baseline uses another toolchain, build selection or mapping rule
- **THEN** comparison fails and the baseline remains unchanged

### Requirement: Certification SHALL retain real evidence and limits
Positive certification SHALL use real compiled native output with retained raw
artifacts. Negative validation SHALL detect omissions, ambiguity, duplication and
tampering. Minimal fixtures SHALL NOT imply complete Arc-Admin backend certification.

#### Scenario: Backend cannot be fully measured
- **WHEN** real backend mapping or host capability is missing
- **THEN** the exact failure and original evidence are retained
- **AND** no completion declaration or GH-221 readiness is asserted

## Implementation plan and timeline
Within this issue, execute N0/N1 first, then N2/N3, N4/N5 and N6. Each evidence or
implementation chunk in tasks.md is bounded to less than four hours; unresolved
compiler boundaries require further design and cannot be marked accepted on time
spent. Acceptance requires all four requirements above, not only the prototype.

The production development collector is `tools/quality/rust_native_driver.py`.
Its pinned compiler driver retains all post-analysis definitions, constant MIR,
recursive expansion edges and independent MIR block counters. See
[design](../../design.md) and [reproduction instructions](../../../../../tools/quality/rust-native-driver/README.md).
The previous `rust_native.py` fixture adapter remains explicitly experimental;
its old reports do not gain production authority.

#### Scenario: Generated methods disabled by stock coverage
- **WHEN** a derive marks a method coverage(off), or a macro generates a constant/type
- **THEN** independent method counters or retained compile-time MIR/declarations establish its compiler role
- **AND** no declaration or owner is silently removed from the inventory

#### Scenario: Unchanged debt and new regression
- **WHEN** compatible real captures have unchanged historical CRAP debt
- **THEN** the existing Rust evaluator reports and retains that debt
- **AND** newly introduced debt or regression still fails without resetting the baseline

Alternatives rejected: source AST absence as proof, arbitrary expression fallback,
parent counters for closures, unfiltered file summaries relabeled as production,
and invented JSON positives. Stock coverage omits generated owners; the pinned
compiler driver provides independent counters instead of waiving those omissions.

Rollback: remove the opt-in production adapter/driver without changing the old
reference series, accepted baseline or project gates. Retain raw evidence and
failure records even if the development collector is rolled back.
