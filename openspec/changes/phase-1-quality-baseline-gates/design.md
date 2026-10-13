# Design: Phase 1 Quality Baseline Gates

## Scope Boundary

This document defines contracts for the quality-evidence implementation. It is
not product business logic and does not change the behavior of Harness-Gate.
The implementation wraps the existing binary with test and CI orchestration;
the inputs, outputs, invariants, and acceptance rules remain the source of truth.

## System Model

The quality flow is an evidence pipeline around the existing CLI:

```text
declared boundaries and fixtures
        -> reproducible runners
        -> raw evidence
        -> normalized summaries
        -> blocking gate evaluation
        -> reviewable artifact package
```

Each runner has one documented local invocation and one CI invocation.
The two invocations must differ only in workspace paths, credentials for
artifact upload, and CI's non-interactive environment. A runner must fail
closed when required metadata or an evidence file is missing.

## Evidence Package

The implementation publishes one package with these logical parts:

| Part | Minimum contents | Required properties |
| --- | --- | --- |
| Metadata | commit SHA, branch/event, OS, CPU, memory, target triple, Rust/Cargo/tool versions, harness version | Machine-readable; immutable for the run. |
| Coverage | per-module executable and covered lines, aggregate, exclusions, command, tool versions | Recomputable percentages; no test-name inference. |
| Critical paths | stable row IDs, applicability, owner, linked test, expected outcome, observed outcome, status, evidence paths | Every applicable row is auditable. |
| Benchmarks | raw samples, summary statistics, cache state, fixture version, report-derived step durations | Cold/warm distinction is explicit. |
| Compatibility | scenario, arguments, exit status, normalized stdout/stderr, reports, snapshot version | Normalization is narrow and deterministic. |
| Documentation | link results, example inventory/load results, migration results, schema generation/diff/validation results | Source and generated artifacts are identified. |

Machine-readable summaries should be stable enough for a later dashboard, while
raw files retain the details needed to investigate a failed gate. Human-readable
Markdown is a rendering of the same data, not a second source of truth.

## 1. Coverage Contract

### Declared Source Boundaries

The initial module boundaries are fixed to the following paths. The first six
boundaries plus `secrets` are blocking; `service` is informational because its
Docker adapter requires an external daemon.

| Module ID | Source path |
| --- | --- |
| `config` | `tools/harness-gate/src/config/**` |
| `verify` | `tools/harness-gate/src/verify/**` |
| `process` | `tools/harness-gate/src/process/**` |
| `service` | `tools/harness-gate/src/service/**` |
| `audit` | `tools/harness-gate/src/audit/**` |
| `scope` | `tools/harness-gate/src/scope/**` |
| `secrets` | `tools/harness-gate/src/secrets/**` |

The exclusion list is versioned and reviewed. It may exclude generated files,
test sources, `target/`, and presentation-only code only when the report names
the excluded path and rationale. An empty executable-line set is reported as
`N/A` and requires an explicit review note; it is never silently omitted.

### Measurement Invariants

- The coverage tool runs with the locked dependency graph and a recorded tool
  version.
- The source-path summary is derived from coverage data, not from test names or
  module-specific test counts.
- Every declared module has an individual percentage and contributes to the
  aggregate according to executable-line counts.
- The gate fails if the summary, exclusions, commit SHA, or tool metadata is
  missing.

### Acceptance Formula

```text
module coverage = covered executable lines / executable lines * 100
aggregate coverage = sum(covered executable lines) /
                     sum(executable lines) * 100
```

Both every applicable module and the aggregate must be at least 80.0%.

## 2. Critical-Path Matrix Contract

The matrix is a declarative inventory. Each row contains:

| Field | Requirement |
| --- | --- |
| `id` | Stable identifier that survives test renames. |
| `owner_module` | One declared source boundary. |
| `stimulus` | Observable input, signal, timeout, invalid config, or injected failure. |
| `expected` | User-visible and/or resource-state outcome. |
| `test` | Fully qualified automated test identifier. |
| `platforms` | `linux`, `macos`, `windows`, or an explicit applicability rule. |
| `evidence` | Paths to test result, report, log, or cleanup proof. |
| `status` | `pass`, `fail`, or `not-applicable` with a reason. |

The initial inventory must include process cancellation, timeout, non-zero
steps, service startup/readiness/cleanup failures, built-in gate failures,
scope/configuration failures, parser failures, and report-write failures.
There must be explicit rows for process-tree cleanup, cancellation, and report
integrity.

The gate is computed as:

```text
passing traceable applicable rows /
total applicable rows * 100 >= 95.0%
```

A row is traceable only when its test ran, its expected observable outcome was
asserted, its owner boundary has non-zero coverage, and its evidence path is
present. A failed or untraceable row counts against the threshold. The three
mandatory safety categories may not be waived by the percentage.

## 3. Benchmark and Binary Contract

### Fixture Constraints

The benchmark fixture is deterministic, checked in, and independent of network
availability and Docker. It runs a successful `verify --all` with at least two
configured steps and produces the normal verification report and per-step logs.
The fixture version is part of the comparison key.

### Required Measurements

| Measurement | Sampling protocol | Recorded result |
| --- | --- | --- |
| Verification wall time | Five warm runs using the release-small binary | Raw samples, median, min, max, command, fixture version. |
| Step duration | Same five reports | Label, `duration_ms`, pass state, log path for each step and run. |
| Test duration | One cold and five warm locked nextest runs | Raw samples, median, min, max, cache state, test count. |
| Binary size | Locked release-small build | Exact bytes, target triple, SHA-256, build metadata. |

The runner records whether compilation and dependency caches were warm. It may
remove only its dedicated output directory and must not clean a developer's
working tree or shared target directory destructively.

### Comparison Rules

- Compare medians only within the same target triple, toolchain series, fixture
  version, and harness version.
- A change greater than 15% in verification median, test median, or binary bytes
  is a blocking regression.
- A new comparison series is reported, not rejected, when its comparison key is
  incompatible with the accepted baseline.
- A Markdown report must link each summary value to its raw sample file.

## 4. Compatibility Snapshot Contract

### Scenario Set

The initial scenarios cover help/version, presets, initialization, schema
export, valid and invalid configuration, scope, secrets, audit, successful and
failed verification, cancellation, one-step execution, and unknown
command/profile/step failures.

When applicable, scenarios assert the preserved paths:

```text
scope.json
secret_scan.json
review_context.json
review_context.md
test_result.json
test_result.md
logs/<step-log>
```

### Capture Rules

- Use checked-in fixtures, a controlled project root, `--color never`, fixed
  locale/timezone, and deterministic environment variables.
- Capture numeric exit status, complete stdout, complete stderr, all reported
  paths, path existence, and normalized JSON/Markdown reports.
- Normalize only the fixture's absolute root, RFC 3339 timestamp, and measured
  duration. Replace them with named tokens; do not normalize wording, file
  names, error codes, ordering, or report structure.
- Linux owns the textual golden snapshots. All supported platforms run the same
  scenarios as structured contract tests for status, error code, relative paths,
  JSON shape, and absence of ANSI escapes.
- Snapshot acceptance is a reviewed operation and cannot be enabled in CI.

The snapshot suite is a compatibility boundary, not a test of internal
implementation details. Any intentional public change must update the related
ADR/OpenSpec record and include an explicit snapshot diff review.

## 5. Documentation, Example, and Schema Contract

The future consistency check runs regardless of whether a Markdown file changed.
It must:

1. Resolve local Markdown links and fragments in the two READMEs,
   `CONTRIBUTING.md`, `CHANGELOG.md`, and `docs/**/*.md`.
2. Check external links on a scheduled job with bounded retries and a reviewed
   allowlist for unavailable endpoints.
3. Discover every embedded preset, checked-in example, and TOML block referenced
   by documentation through a linked fixture or extraction manifest.
4. Load each v2 example through the CLI and exercise each documented v1
   migration example. Environment-variable examples use an explicit manifest,
   including one missing-variable failure case.
5. Generate the schema into a temporary location, compare it byte-for-byte with
   `schema/flow.schema.json`, and validate each v2 example against the generated
   schema.

The committed schema is an output of the configuration model, never a manually
edited source. A schema diff is a gate failure until the associated contract
change is reviewed.

## 6. CI Gate Topology

The future CI jobs are logically separated so a failure identifies the evidence
class without changing application behavior:

| Logical job | Required output |
| --- | --- |
| `quality-coverage` | Module table, aggregate coverage, critical-path matrix. |
| `quality-contracts` | CLI/report/error snapshots and cross-platform structured results. |
| `quality-baseline` | Benchmark samples, summary, comparison, binary hash/size. |
| `docs-consistency` | Links, examples, migrations, schema generation/diff/validation. |

All jobs use the locked manifest and versioned tool/action revisions. They write
to one predictable quality-artifact root and upload results on failure as well
as success. Required-check status is the merge boundary; hosted dashboards are
supplementary.

## Implementation Plan And Delivery Status

The following sequence tracks this delivery. Baseline acceptance and owner
review are complete; the evidence and governance records below are the source of
truth for closeout:

| Phase | Timebox | Deliverable |
| --- | --- | --- |
| A. Inventory | complete | Module boundaries, exclusions, public command/report/error inventory. |
| B. Contract definition | complete | Versioned matrix, artifact schemas, snapshot normalization rules, benchmark comparison key. |
| C. Evidence runners | complete | Local/CI-parity runners for coverage, critical paths, benchmarks, contracts, and docs. |
| D. Baseline capture | complete | Accepted `main` evidence package and initial history entry. |
| E. Required checks | complete | Blocking CI mapping and failure artifact upload are present; organization ruleset `21098892` requires `Required Quality Aggregate` with strict status checks on the default branch. |
| F. Review | complete | Cross-platform confirmation and the accepted `main` evidence package are green; ruleset applicability is confirmed by the effective-rules API. |

Each phase requires its own reviewable artifact and must stop if it would alter
runtime behavior or a public contract without a separate decision record.

## Accepted Governance Evidence

The accepted `main` evidence is [CI run
33466540997](https://github.com/musutrade/Harness-Gate/actions/runs/33466540997)
for commit `6e17867cf42139a3ecd73d014dc566d21d08d562`. It passed the four
quality jobs, all cross-platform structured jobs, and `Required Quality
Aggregate`. Organization ruleset
[`21098892`](https://github.com/musutrade/Harness-Gate/rules/21098892) is
`active`, includes `~DEFAULT_BRANCH`, requires `Required Quality Aggregate`,
and sets `strict_required_status_checks_policy: true`; the effective rules for
`main` are returned by the [rules-for-main API](https://api.github.com/repos/musutrade/Harness-Gate/rules/branches/main).

The required-check configuration is accepted. A deliberately failing pull
request has not been run and remains an optional operational validation; no
green run is described as that negative test.

## Alternatives Considered

### Keep only the current repository-wide Codecov threshold

Rejected. A global number can conceal a weak process or service boundary, and a
hosted status does not provide a complete local evidence package.

### Require 100% coverage

Rejected. It incentivizes implementation-detail tests and exclusions. The
80%-per-module threshold is paired with a stricter matrix for safety-critical
outcomes.

### Use loose CLI substring assertions

Rejected. Substrings do not protect exit status, exact error codes, report paths,
serialized report shape, or output ordering relied upon by scripts.

### Capture one manual benchmark before each release

Rejected. It omits per-step cost, has no comparable cache/environment protocol,
and detects regressions too late.

### Run documentation checks only for documentation changes

Rejected. Code, presets, configuration types, and generated schema can invalidate
documentation without changing a Markdown file.

## Rollback Plan

Rollback is a revert of the quality tooling, workflow, and specification files.
It has no product runtime, configuration semantics, or user-data effect.

After future implementation, rollback must be track-scoped: disable the affected
required check while retaining artifact publication, revert only the runner or
manifest commit, and preserve the last accepted baseline. Snapshot and schema
changes must not be silently discarded; they require an explicit reviewed
reversion record.

## Acceptance Criteria for This OpenSpec

- `proposal.md`, `design.md`, and `tasks.md` exist under one change directory.
- Goals, non-goals, metrics, risk, alternatives, rollback, and related records
  are explicit.
- Thresholds, formulas, evidence fields, platform rules, and update governance
  are unambiguous.
- No production Rust, test, CI, fixture, snapshot, schema, or generated-file
  implementation is included.
- All local references resolve, and implementation/acceptance status is tracked
  in `tasks.md`; accepted governance evidence and optional operational follow-up
  are explicit.
