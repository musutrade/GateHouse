# Verification Experience

## ADDED Requirements

### Requirement: Human configuration repair guidance

The default human `config check` command SHALL report a stable error category,
field diagnostics when available, and a safe next-step repair command. It SHALL
include a minimal valid schema v2 flow shape without resolved secrets or other
environment values.

#### Scenario: Missing configuration suggests a safe repair

- **WHEN** a user runs `harness-gate config check` without a valid workflow
  configuration
- **THEN** stderr includes the `E1000` category, the generic init command, and
  a minimal `version = 2` flow shape

### Requirement: Gate-overlapped service startup

Verification SHALL start only services referenced by selected external plan
nodes in parallel with mandatory secret and architecture gates. Warmup workers
MUST be joined before returning, and failed startup MUST remain observable to
the subsequent service lease.

#### Scenario: Selected service starts while gates run

- **WHEN** a selected external step declares a service and verification begins
- **THEN** an owned warmup worker may start that service before the gates finish
- **AND THEN** the first lease observes the ready resource or its cached startup
  failure

### Requirement: Isolated matcher cache evidence

The quality baseline SHALL measure cached and uncached scope classification for
the same paths in one process, verify equivalent results, and publish per-sample
timings and speedup without mixing process startup or Git discovery into the
classification measurement.

#### Scenario: Benchmark proves equivalent cached results

- **WHEN** the quality benchmark runs against its generated changed paths
- **THEN** cached and uncached classifications are repeated in one process
- **AND THEN** the output records per-iteration timings, speedup, and an
  equivalence flag
