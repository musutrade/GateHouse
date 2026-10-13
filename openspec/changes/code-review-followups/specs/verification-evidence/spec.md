# Verification Evidence

## MODIFIED Requirements

### Requirement: Scope classification reuses compiled matchers

Project discovery MUST compile each configured scope rule once and scope
classification MUST reuse those matchers for the lifetime of the discovered
project. Standalone configuration helpers MAY compile temporary matchers for
callers that do not own a `Project`.

#### Scenario: Repeated scope detection uses one compiled rule set

- **WHEN** a discovered project performs scope detection more than once
- **THEN** the configured glob patterns are not rebuilt for each invocation
- **AND** component and unmatched-path results remain identical to direct classification

### Requirement: Dependency-blocked steps are observable

Verification reports MUST expose dependency-blocked nodes with their stable ID,
label, and blocking reason. Successful reports MUST omit an empty skipped list
to preserve the existing successful JSON shape. Markdown and JUnit reports MUST
render the skipped status without counting it as an executed failure.

#### Scenario: A failed prerequisite blocks a descendant

- **WHEN** an external step fails and a dependent step is not dispatched
- **THEN** JSON contains the dependent step under `skipped_steps`
- **AND** Markdown contains a `SKIPPED` entry
- **AND** JUnit contains a `<skipped>` testcase for that node

### Requirement: Performance evidence is platform-scoped

The quality baseline workflow MUST run on Ubuntu, macOS, and Windows and MUST
publish artifacts whose names identify the runner platform. Baseline comparisons
MUST remain within matching target, toolchain, fixture, and cache-state series.

#### Scenario: Supported platforms produce independent baseline artifacts

- **WHEN** the quality baseline workflow runs for a pull request
- **THEN** each supported runner captures verification timing and release-small
  size evidence
- **AND** each uploaded artifact identifies its platform
- **AND** no platform's measurements are compared numerically with another's

#### Scenario: Benchmark fixture runs without POSIX-only primitives

- **WHEN** the benchmark fixture runs on Windows
- **THEN** it uses the invoking Python interpreter and a portable exclusive-create
  lock
- **AND** the worker records zero leaked active workers before the sample is accepted
