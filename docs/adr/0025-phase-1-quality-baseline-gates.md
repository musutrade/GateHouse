# ADR-0025: Establish Phase 1 Quality Baseline Gates

## Status

**Accepted** (2026-09-01)

## Context

The planned refactoring changes the configuration, verification, process, and
service boundaries. It must begin with reproducible evidence of current
behaviour and cost; a passing test suite or a repository-wide coverage number
alone does not show whether the risky paths remain protected.

The repository already has cross-platform tests, Codecov reporting, generated
schema verification, historical Phase 2 benchmarks, and several targeted CLI
contract tests. Those checks do not yet provide all of the following as
reviewable, repeatable gates:

- a per-core-module coverage threshold;
- an explicit, auditable measure for cancellation and failure paths;
- comparable verification, step, test, and distribution-binary measurements;
- byte-for-byte-compatible CLI, report-path, exit-status, and report-shape
  contracts; and
- one blocking automation boundary for documentation links, loadable examples,
  and the generated JSON Schema.

Without these gates, a structural change can preserve a few happy-path tests
while regressing cancellation cleanup, error classification, report locations,
or scripts that consume the CLI. Performance claims can also become
non-comparable when toolchain, fixture, cache state, or measurement method is
not recorded.

## Decision

Phase 1 will create a versioned quality-baseline harness and make its defined
gates blocking for pull requests and the `main` branch. The harness must be
runnable locally with the same commands used in CI and must publish its raw
artifacts, machine-readable summaries, and a human-readable report.

The gates below define the required implementation and the acceptance evidence.
An exception is valid only when it names a tracked issue, has an expiry date,
and is approved by the code owner; exceptions do not alter the thresholds.

### 1. Coverage Boundary and Thresholds

The Phase 1 blocking core modules are:

| Module | Source boundary | Reason |
| --- | --- | --- |
| Configuration | `src/config/**` | Parses and validates the public workflow contract. |
| Verification | `src/verify/**` | Selects gates, executes steps, and writes the verification report. |
| Process | `src/process/**` | Owns timeouts, subprocess lifetime, and cancellation. |
| Audit | `src/audit/**` | Implements an included gate and report output. |
| Scope | `src/scope/**` | Selects the work that verification will run. |
| Secrets | `src/secrets/**` | Implements the credential gate and staged-file behaviour. |

`src/service/**` remains a declared external-adapter boundary. Its coverage is
reported as informational because Docker startup/readiness/cleanup requires an
external daemon; service contract tests cover environment and external-value
paths locally, while Docker-specific evidence is collected on capable runners.

Use `cargo llvm-cov` on Linux with the locked toolchain to produce Cobertura,
LCOV, and JSON coverage artifacts. The quality harness derives coverage from
the JSON result by source path; it must not infer module ownership from Rust
test names. Generated files, test sources, `target/`, and UI-only rendering are
excluded only through a reviewed, versioned exclusion list.

Each blocking core module must have at least **80.0% line coverage** and the aggregate
of those modules must also be at least **80.0%**. The service adapter is listed separately as informational and cannot improve the blocking aggregate. A module with no executable
lines is reported as `N/A` and requires an explicit review note; it cannot be
silently omitted. The coverage report records covered lines, executable lines,
percentage, the commit SHA, command, tool versions, and exclusion-list version.

Codecov may continue to display coverage trends, but its availability is not
the source of truth for a merge decision. CI evaluates the locally generated
JSON summary before attempting any external upload.

### 2. Cancellation and Failure-Path Evidence

Line coverage is insufficient to prove safety-critical outcomes. Maintain a
versioned critical-path matrix, for example
`tools/harness-gate/tests/quality/critical-paths.toml`, containing a stable ID,
owner module, stimulus, expected observable outcome, and test name for every
path below:

| Area | Required observable outcome |
| --- | --- |
| Process cancellation | Signal stops the child process tree, marks the task cancelled, and returns `E1402`. |
| Process timeout | Timeout terminates the child process tree, records `timed_out`, and retains the step log. |
| Non-zero external step | Failure retains the exit-code detail and log path; later configured steps obey the documented stop policy. |
| Service startup, readiness, and cleanup failure | Failure is attributed to the requesting step, returns a reportable result, and leaves no managed container or process running. |
| Built-in gate failure | Secrets and audit failures stop configured steps and still write the compatible verification report. |
| Scope/configuration failure | Invalid, missing, or cyclic input returns the documented coded error before unsafe execution starts. |
| Parser and report-write failure | Parse failure marks the step failed; report-write failure returns `E1404` without misrepresenting verification status. |

Each matrix row must point to an automated test that exercises the actual
boundary, not a mock of the result object. The test records all applicable
observable values: process exit status, `ERROR [E####]` text, task fields,
report path, report existence, and cleanup state. Platform-specific process
assertions may use platform implementations, but the same matrix ID must be
verified on every supported platform or explicitly marked Linux-only with a
reason and an expiry date.

The harness calculates critical-path evidence coverage as:

```text
passed, traceable applicable matrix rows / all applicable matrix rows * 100
```

The result must be at least **95.0%**, with no untested row for process-tree
cleanup, cancellation, or report integrity. A row is traceable only when its
test appears in the test result and its source boundary has non-zero coverage
in the coverage artifact. The generated matrix report includes all IDs,
applicability, linked test, pass/fail status, and evidence locations, allowing
a reviewer to recompute the percentage.

### 3. Reproducible Performance and Size Baselines

Add a deterministic benchmark fixture that contains no network dependency,
requires no Docker daemon, uses a fixed project configuration, and runs a
small, representative successful `verify --all` workflow. The fixture must
exercise at least two configured steps so that total verification time and
individual `TaskResult.duration_ms` values are meaningful.

The baseline command records the following metrics:

| Metric | Command or source | Required recording |
| --- | --- | --- |
| Verification wall time | Release-small binary against the benchmark fixture | Five warm samples, median, min, max, and per-step durations from `test_result.json`. |
| Step duration | `TaskResult.duration_ms` from the same report | Each step label, duration, pass state, and log path. |
| Test time | `cargo nextest run --manifest-path tools/harness-gate/Cargo.toml --locked` | One cold sample and five warm samples, with median/min/max. |
| Release binary size | `cargo build --profile release-small --locked` | Exact byte count of the shipped target binary, target triple, and SHA-256. |

The runner starts each sample from a documented state. It records whether a
build cache was warm, deletes only its dedicated benchmark output directory,
uses `--locked`, and captures the OS, CPU model, logical CPU count, memory,
Rust and Cargo versions, target triple, commit SHA, and benchmark-harness
version. It must not use `cargo clean` against a developer's working tree.

Results are written in both a stable machine-readable form, such as
`docs/benchmarks/phase-1/current.json`, and a reviewable Markdown report. Raw
per-run files are uploaded as CI artifacts. Every pull request publishes a
candidate report and comparison with the last accepted `main` baseline; a
scheduled weekly `main` run and a manual dispatch create a reviewable automated
pull request updating the canonical current and history artifacts. This makes
measurements continuously refreshed without allowing an unreviewed runner
change or transient machine result to rewrite the reference baseline.

Phase 1 establishes a baseline, not a speculative performance target. A change
of more than 15% in median verification time, test time, or binary bytes is a
blocking regression unless the pull request includes an approved ADR or
benchmark note that explains and accepts the trade-off. The comparison is only
valid for matching target triple and benchmark-harness version; otherwise it is
recorded as a new baseline series rather than a regression.

### 4. CLI, Report, and Error Compatibility Snapshots

Add golden integration snapshots for the public command contract. They use
checked-in fixtures, `--color never`, a fixed locale and timezone, a controlled
project root, and a normalizer that replaces only the fixture's absolute root,
RFC 3339 timestamp, and measured duration with named tokens. Paths are then
asserted relative to `<PROJECT_ROOT>`; filenames, directory layout, formatting,
and all non-dynamic content remain literal contract data.

For each snapshot scenario, capture and assert all of:

- command arguments and fixture setup;
- numeric process exit status;
- complete stdout and stderr after the narrowly defined normalization;
- exact error-code form (`ERROR [E####]`) on failures;
- every reported output path and existence of the output file; and
- the complete normalized JSON and Markdown report where the command writes a
  report.

The initial contract suite covers help/version, preset listing and initialization,
schema export, valid and invalid configuration, scope, secret scanning, audit,
verification success, verification failure, cancellation, one-step execution,
and unknown command/profile/step errors. It also covers the preserved report
names `scope.json`, `secret_scan.json`, `review_context.json`,
`review_context.md`, `test_result.json`, `test_result.md`, and per-step logs as
applicable to their commands.

Snapshots run on Linux as the authoritative textual golden set. Ubuntu, macOS,
and Windows run the same scenarios as structured contract tests, including exit
status, error code, report-relative paths, JSON shape, and no ANSI escapes.
Platform-specific textual snapshots are permitted only where the platform's
public output is intentionally different and the difference is documented in
the snapshot metadata. Snapshot updates require an explicit review of the
diff; a `--accept` or equivalent update command must never run in CI.

### 5. Documentation, Examples, and Schema Synchronization

Create one blocking documentation-consistency job with these checks:

1. Check every local Markdown link and fragment in `README.md`,
   `README.zh-CN.md`, `CONTRIBUTING.md`, `CHANGELOG.md`, and `docs/**/*.md`.
   Check external links on the scheduled job with bounded retries and a
   reviewed allowlist for endpoints that cannot be queried automatically.
2. Load and validate every supported example configuration: every embedded
   preset, each checked-in example fixture, and every full TOML example
   referenced by documentation. Documentation examples must either be a linked
   fixture or be extracted by the check, so a copied TOML block cannot drift.
3. Run `harness-gate config check` for each v2 example and the documented
   migration command for each v1 migration example. Environment-variable
   examples run with an explicit environment manifest and include at least one
   missing-variable failure assertion.
4. Generate the schema with the locked toolchain into a temporary path and
   compare it byte-for-byte with `schema/flow.schema.json`. Then validate each
   v2 example against that generated schema as well as loading it through the
   CLI.

The existing CI schema check is retained until this job replaces it. The new
job is authoritative because it validates generation, committed synchronization,
and consumer usability in one execution.

### 6. CI and Evidence Layout

The implementation adds clearly named, independently rerunnable jobs:

| Job | Blocking evidence |
| --- | --- |
| `quality-coverage` | Core-module coverage summary and critical-path matrix report. |
| `quality-contracts` | CLI/report/error snapshots and cross-platform structured contract results. |
| `quality-baseline` | Benchmark/size JSON, Markdown comparison, and raw samples. |
| `docs-consistency` | Link results, example-load results, and schema diff. |
| `quality-required` | Always-run aggregate that fails unless all four quality jobs succeed. |

All jobs use the locked manifest and pin tool versions in a versioned quality
tool manifest or action revision. Their output goes under a single predictable
directory, such as `tools/harness-gate/target/quality/`, before upload. CI
uploads artifacts even when a job fails so a reviewer can inspect incomplete
evidence. Required checks protect `main`; Codecov and other hosted dashboards
remain supplementary.

## Consequences

### Positive

- Refactoring begins from observable compatibility and cost contracts instead
  of relying on informal expectations.
- Per-module coverage exposes weak ownership boundaries that a global number
  can hide.
- The critical-path matrix proves the cancellation, cleanup, report, and
  failure behaviours most likely to regress during scheduler work.
- Versioned baselines make release-size and developer-feedback trade-offs
  comparable over time.
- Documentation, examples, configuration schema, and executable behaviour are
  checked as one deliverable.

### Negative

- CI requires additional Linux coverage and benchmark execution time, artifact
  storage, and maintenance of fixtures and snapshots.
- Exact snapshots make intentional user-facing wording and path changes more
  deliberate; maintainers must review and update contract data explicitly.
- Benchmark values remain hardware-sensitive, so the workflow distinguishes
  valid within-series regressions from new measurement series.
- The initial cataloging of failure paths and documentation examples requires
  focused engineering work before later refactoring can begin.

## Alternatives Considered

### Keep the existing repository-wide Codecov target

Rejected. A repository-wide threshold can rise while `process` or `service`
loses coverage. Hosted status alone also does not preserve a complete local,
reviewable measurement artifact.

### Require 100% code coverage

Rejected. It would encourage testing implementation details and blanket
exclusions rather than meaningful boundary coverage. The 80% module threshold
is complemented by the stricter, explicitly traceable 95% critical-path
evidence requirement.

### Measure only `cargo test` and release size manually before releases

Rejected. It omits the user-visible verification workflow and individual step
costs, produces incomparable one-off numbers, and finds regressions too late.

### Use loose assertion tests instead of snapshots

Rejected. Substring assertions do not protect CLI formatting, report paths,
error codes, or serialized report contracts consumed by scripts and CI.

### Run documentation checks only when documentation files change

Rejected. Changes to CLI code, configuration types, presets, and schema can
invalidate documentation without modifying a Markdown file.

## Rollout and Acceptance

Phase 1 is complete only when all four required CI jobs are required checks and
a green run supplies the following reviewable artifacts:

1. A core-module coverage table showing every listed module and an aggregate at
   or above 80.0%.
2. A critical-path matrix report showing at least 95.0% traceable, passing,
   applicable rows and no missing cancellation, cleanup, or report-integrity
   evidence.
3. A baseline report containing verification, step, test, and release-small
   binary measurements with environment metadata and a retained raw artifact.
4. Approved CLI/report/error golden snapshots and passing cross-platform
   contract results.
5. Passing local-link, example-load, migration-example, schema-generation,
   schema-diff, and schema-validation checks.

The resulting artifacts are the evidence package required before Phase 2
configuration or scheduler changes can be accepted.

## Current Acceptance Record

The first accepted `main` evidence package is recorded in
`docs/benchmarks/phase-1/README.md` and was produced by
[CI run 33223804928](https://github.com/musutrade/Harness-Gate/actions/runs/33223804928)
for commit `b92755f9ec87c251516863a0136c28f811187ab5`. The run passed all
cross-platform tests, quality jobs, and the `Required Quality Aggregate`; its
raw baseline artifact is retained by GitHub Actions.

The documentation and acceptance-record closeout is reviewed in
[PR #39](https://github.com/musutrade/Harness-Gate/pull/39), whose required
quality aggregate and cross-platform checks are green.

The organization ruleset [`21098892`](https://github.com/musutrade/Harness-Gate/rules/21098892)
is active and targets the default branch through `ref_name.include: ["~DEFAULT_BRANCH"]`.
The rules that apply to `main` include the `Required Quality Aggregate` status
check with `strict_required_status_checks_policy: true`, as confirmed by the
[rules-for-main API response](https://api.github.com/repos/musutrade/Harness-Gate/rules/branches/main).
The latest `main` run
[33466540997](https://github.com/musutrade/Harness-Gate/actions/runs/33466540997)
for commit `6e17867cf42139a3ecd73d014dc566d21d08d562` passed all quality jobs
and the aggregate. The legacy branch-protection endpoint is not the source of
truth for this organization-level ruleset.

This closes the Phase 1 required-check governance acceptance. The accepted
baseline and CI evidence are defined by the clean-checkout artifacts above;
developer-worktree-only files are not part of the baseline.

## References

- [Function risk evidence](../quality/function-risk.md) implements OpenSpec tasks
  4.1–4.5 with versioned LLVM source mapping, exact incremental CRAP decisions,
  separate historical debt and non-waiving exceptions. Its
  [GH-92 validation record](../quality/gh-92-validation.md) validates the tooling;
  it does not replace the accepted baseline or accept the remaining OpenSpec work.
- [ADR-0003: Enhance CI pipeline](0003-enhance-ci-pipeline.md)
- [ADR-0004: Add integration tests](0004-add-integration-tests.md)
- [ADR-0006: Test fixtures and internal boundaries](0006-test-fixtures-and-internal-boundaries.md)
- [ADR-0015: Decompose the CLI entry module](0015-main-module-decomposition.md)
- [ADR-0023: Generate configuration schema](0023-config-schema-and-interpolation.md)
- [Phase 2 baseline metrics](../benchmarks/phase-2-baseline.md)
