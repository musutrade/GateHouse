## ADDED Requirements

### Requirement: Reuse one build for isolated critical-path tests

A collection SHALL build instrumented test binaries once and reuse nextest build
metadata for every applicable exact test. The default test concurrency SHALL be
two, with an explicit serial option and a supported range of one to eight workers.
The engineering-policy semantics, source-v2 identities, required paths, thresholds
and probe validation SHALL remain unchanged.

#### Scenario: Multiple paths share binaries without sharing counters
- **WHEN** a collection runs multiple applicable critical paths
- **THEN** tests reuse the same completed instrumented build
- **AND** each test and its child processes write to a fresh private profile directory
- **AND** report exports run serially using only the selected test's profiles
- **AND** build-time counters and other tests' counters are excluded

#### Scenario: Collection fails or inputs change
- **WHEN** building, testing, profile preparation or export fails, profiles are missing, or source identity changes
- **THEN** the collector retains diagnostic evidence and publishes no completed bundle
- **AND** the failed build tree is retained for diagnosis up to a bounded number of recent failures
- **AND** no retry or missing result can become a passing mandatory path

#### Scenario: Successful collection releases temporary build storage
- **WHEN** all applicable paths have completed and source identity is verified
- **THEN** the collector removes its owned temporary build directory
- **AND** retains per-test evidence, command results and timing metadata
