# CI Execution Topology

## ADDED Requirements

### Requirement: CI optimization SHALL preserve required assurance semantics

The system SHALL treat the current event-applicable required checks and their semantic outcomes as the compatibility oracle for this change. Execution topology, setup, caching, and artifact reuse MAY change, but the change SHALL NOT remove, downgrade, conditionally skip, or weaken a currently required assurance outcome.

For this change, full macOS and Windows tests SHALL remain required on pull requests. CRAP, coverage, complexity/risk, critical-path, CLI-contract, documentation, release-governance, security, format, Clippy, build, quality-script, measurement-series, debt/ratchet, and Rust authority semantics SHALL remain unchanged.

#### Scenario: Optimization attempts to skip cross-platform tests on an ordinary PR
- **GIVEN** macOS and Windows full tests are currently PR-required
- **WHEN** an optimization proposes path-based or sampled execution that would skip either platform
- **THEN** the optimization is rejected from this change
- **AND** it requires a separate normative Engineering Policy delta with assurance evidence

#### Scenario: Existing required quality semantics remain after topology changes
- **GIVEN** a pull request evaluated before and after the optimization
- **WHEN** the same semantic failure is introduced into a currently required gate
- **THEN** the optimized workflow still produces a blocking required failure
- **AND** no threshold, requiredness, measurement series, or authority boundary is weakened

### Requirement: Hosted CI cost SHALL be measured with comparable before/after evidence

The change SHALL retain normalized hosted GitHub Actions evidence for representative successful before-state and after-state pull-request runs. The records SHALL bind workflow/run/attempt/source identity and SHALL report critical path, job wall times, observable setup/tool-install time, quality collection time, and approximate runner wall minutes separated by operating system.

Local timing SHALL NOT be presented as hosted-run performance evidence.

#### Scenario: Optimization claims improved PR latency
- **GIVEN** a proposed optimization has completed successfully
- **WHEN** performance improvement is claimed
- **THEN** the claim is supported by comparable hosted before/after run records
- **AND** the report distinguishes developer critical path from total runner work
- **AND** noisy single-run data is not represented as a guaranteed percentage improvement

### Requirement: CI tool installation SHALL be explicit, pinned, and fail closed

Required CI tools SHALL have explicit version identity. The workflow SHOULD use verified/prebuilt installation or validated durable caches instead of repeated forced source compilation when that preserves the same tool contract. Cache misses or installation failures SHALL NOT skip the required gate.

#### Scenario: Cached quality tool is absent
- **GIVEN** a required job expects a pinned quality tool
- **AND** its binary cache is missing or invalid
- **WHEN** the job starts
- **THEN** it uses the trusted pinned installation fallback
- **OR** fails the job if installation cannot be completed
- **AND** it never marks the required gate successful without the tool

#### Scenario: Tool version drifts
- **GIVEN** a required tool version is pinned by the CI contract
- **WHEN** the installed executable reports an incompatible version
- **THEN** the job fails or reinstalls the expected version
- **AND** does not silently continue with the incompatible tool

### Requirement: Cargo caches and target directories SHALL match actual build state

CI SHALL use explicit or discoverable Cargo target-directory conventions so cache paths match the build outputs they are intended to reuse. Cache identity SHALL distinguish relevant OS, architecture/toolchain/lockfile/profile dimensions where needed to prevent unsafe or ineffective reuse.

#### Scenario: Cargo target directory differs from repository-local default
- **GIVEN** CI configures a non-default `CARGO_TARGET_DIR`
- **WHEN** a Rust cache is restored or saved
- **THEN** the cache targets that actual directory
- **AND** the workflow does not assume `tools/harness-gate/target` merely from manifest location

### Requirement: Authoritative artifact reuse SHALL preserve immutable provenance

A job MAY consume an artifact produced by another job only when the artifact is compatible with the consumer's semantic contract. Authoritative quality artifacts SHALL be bound to expected commit/run/attempt/configuration/tool identity and validated before use. Missing or mismatched required artifacts SHALL fail closed.

Artifact reuse SHALL NOT replace required native cross-platform execution or mix instrumented measurement builds with incompatible release/performance claims.

#### Scenario: Downstream quality consumer receives mismatched retained evidence
- **GIVEN** a downstream consumer expects retained evidence from the current run/attempt
- **WHEN** the downloaded artifact has a different source/run identity or invalid hash
- **THEN** the consumer fails
- **AND** does not recollect silently or fall back to stale/reference evidence as success

#### Scenario: Coverage build is considered for performance reuse
- **GIVEN** an instrumented coverage build exists
- **WHEN** a performance or release-size job needs an uninstrumented product artifact
- **THEN** the instrumented build is rejected as incompatible
- **AND** the native required measurement contract remains unchanged

### Requirement: Provenance-sensitive measurements SHALL have a single collection owner per series

Coverage, risk/CRAP, complexity, and critical-path measurements whose contracts require exact source/tool/instrumentation identity SHALL be collected once by their designated producer for a run/series. Downstream projection, reporting, or compatibility consumers SHALL reuse and validate retained immutable evidence instead of recollecting an allegedly equivalent series.

#### Scenario: Generic consumer needs Rust risk evidence
- **GIVEN** the current run already produced retained authoritative Rust risk evidence
- **WHEN** a generic projection/reporting consumer executes
- **THEN** it consumes and validates that retained evidence
- **AND** does not rerun a second independent CRAP/coverage collection merely for convenience

### Requirement: Required Quality Aggregate SHALL remain a stable lightweight fail-closed aggregator

The workflow SHALL retain the exact check name `Required Quality Aggregate` and its `always()` evaluation behavior. It SHALL evaluate event-applicable child results and fail unless all required children satisfy the existing event policy. The aggregate SHALL NOT compile the product, execute the test suite, recollect quality evidence, or install heavy Rust measurement tools.

#### Scenario: Required child is cancelled or missing
- **GIVEN** a pull request has a required child job
- **WHEN** that child is cancelled, missing, failed, or unexpectedly skipped
- **THEN** `Required Quality Aggregate` fails
- **AND** does not infer success from other green children

#### Scenario: Push-only child is skipped on pull request by existing event policy
- **GIVEN** a child is intentionally push-only under the pre-change event contract
- **WHEN** the workflow runs for a pull request
- **THEN** the aggregate handles that child according to the unchanged event policy
- **AND** this does not authorize skipping any PR-required child

#### Scenario: Required child result is malformed
- **GIVEN** the aggregate receives a non-object needs payload or required child entry
- **WHEN** it evaluates the results
- **THEN** it fails with a bounded diagnostic
- **AND** does not collect replacement evidence or invoke external tools

#### Scenario: Aggregate gains a heavy setup or execution step
- **GIVEN** the aggregate only checks out sources, prepares Python, and evaluates child results
- **WHEN** an extra action or command adds compilation, tests, collection, or Rust tool installation
- **THEN** the aggregate step contract regression test fails

### Requirement: Optimization SHALL consider critical path and total runner work separately

A topology change SHALL NOT be accepted solely because it reduces job count. Acceptance evidence SHALL consider both developer-visible PR critical path and total runner work/setup duplication. An optimization that is neutral/worse or introduces trust ambiguity SHOULD be reverted rather than preserved by weakening assurance.

#### Scenario: Merging jobs saves setup but serializes tests
- **GIVEN** two previously parallel jobs are combined
- **WHEN** hosted after-state evidence shows the PR critical path materially worsens without a compensating required assurance benefit
- **THEN** the merge is reverted or redesigned
- **AND** required assurance is not reduced to recover latency

### Requirement: Future conditional assurance changes SHALL be separate policy changes

Risk-driven conditional cross-platform execution, sampling, scheduled certification, or any other reduction in currently required event coverage SHALL NOT be introduced by this change. Such proposals SHALL identify the exact Engineering Policy delta and provide compatibility/negative evidence independently.

#### Scenario: Later work proposes path-based Windows execution
- **GIVEN** this optimization change has completed
- **WHEN** a later proposal wants Windows tests only for selected paths
- **THEN** it is reviewed as a separate normative policy change
- **AND** the completed topology optimization is not cited as implicit authorization to reduce platform assurance
