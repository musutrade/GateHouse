# Tasks: Phase 1 Quality Baseline Gates

**Parent:** [proposal.md](proposal.md) and [design.md](design.md)
**Status:** Implemented. Local and CI-parity evidence closeout is complete; the
four quality jobs and aggregate are green, and organization ruleset `21098892`
requires the aggregate with strict status checks on the default branch.
**Implementation restriction:** Tasks authorize evidence infrastructure only;
they do not authorize business or production behavior changes.

Each task is intentionally smaller than four hours. Every acceptance criterion
must be demonstrated with a reviewable artifact, and every task must preserve
the existing CLI, report, error-code, configuration, and runtime contracts.

## 1. Contract Inventory

- [x] **1.1 (P0, S)** Inventory the existing CLI commands, arguments, exit
  statuses, error codes, output paths, report names, and report fields.
  **Acceptance:** A reviewed inventory covers every scenario named in
  `design.md`; unknown values are marked as gaps rather than guessed.

- [x] **1.2 (P0, S)** Approve the core-module source boundaries and exclusion
  policy.
  **Acceptance:** Each module has a path owner, every exclusion has a rationale
  and version, and no executable source is excluded implicitly.

- [x] **1.3 (P0, S)** Define the critical-path matrix rows, platform
  applicability, owners, and observable outcomes.
  **Acceptance:** Rows exist for cancellation, process cleanup, timeout,
  service cleanup, gate failure, configuration failure, parser failure, and
  report integrity; each row has a stable ID and no placeholder expected result.

## 2. Evidence Contracts

- [x] **2.1 (P0, S)** Specify the machine-readable evidence metadata and summary
  schemas.
  **Acceptance:** A reviewer can locate commit, tool, target, fixture, cache,
  raw-evidence, and summary fields without relying on Markdown text.

- [x] **2.2 (P0, S)** Specify coverage calculation and critical-path traceability
  rules in a versioned contract.
  **Acceptance:** The 80.0% and 95.0% formulas, `N/A` handling, mandatory safety
  rows, and missing-artifact failure behavior are testable from the contract.

- [x] **2.3 (P0, S)** Specify benchmark sampling, comparison keys, and regression
  rules.
  **Acceptance:** Cold/warm samples, median/min/max, per-step values, binary
  bytes/hash, environment metadata, 15% threshold, and incompatible-series
  behavior are all defined.

- [x] **2.4 (P0, S)** Specify snapshot normalization and update governance.
  **Acceptance:** Only approved dynamic fields are tokenized; exit status,
  error code, paths, report shape, and wording remain protected; CI cannot
  auto-accept snapshot updates.

## 3. Fixture and Inventory Preparation

- [x] **3.1 (P1, S)** Select and version a deterministic no-network,
  no-Docker benchmark fixture with at least two configured steps.
  **Acceptance:** The fixture setup, required environment, expected reports, and
  cleanup scope are documented; no secret or host-specific path is embedded.

- [x] **3.2 (P1, S)** Create the documentation example and migration inventory.
  **Acceptance:** Every embedded preset, checked-in example, linked TOML block,
  and v1 migration example has one stable inventory entry and an owner.

- [x] **3.3 (P1, S)** Define the cross-platform contract scenario matrix.
  **Acceptance:** Linux textual snapshots and Linux/macOS/Windows structured
  assertions are mapped for each command category, including platform-specific
  applicability reasons where needed.

## 4. Local Evidence Runners

- [x] **4.1 (P0, M)** Provide a local invocation that generates coverage artifacts
  and the per-module threshold summary.
  **Acceptance:** The command uses the locked graph, records metadata, fails
  below threshold, and leaves raw artifacts for review.

- [x] **4.2 (P0, M)** Provide a local invocation that evaluates the critical-path
  matrix against executed test evidence.
  **Acceptance:** The report lists every row, applicability, linked test,
  traceability, status, and evidence path; missing mandatory rows fail.

- [x] **4.3 (P1, M)** Provide the benchmark and binary-size runner.
  **Acceptance:** It performs the required cold/warm samples, records per-step
  report values, avoids destructive cleaning, and emits comparable JSON and
  Markdown summaries.

- [x] **4.4 (P0, M)** Provide the CLI/report/error contract runner and narrowly
  scoped normalizer.
  **Acceptance:** It captures exit status, complete streams, report existence,
  normalized reports, and rejects unreviewed snapshot updates.

- [x] **4.5 (P0, M)** Provide the documentation/example/schema consistency
  runner.
  **Acceptance:** Local links/fragments, examples, migrations, schema generation,
  byte-for-byte synchronization, and schema validation are evaluated together.

## 5. Baseline and CI Governance

- [x] **5.1 (P0, S)** Capture the first accepted `main` evidence package.
  **Acceptance:** The package contains all required summaries, raw artifacts,
  environment metadata, reviewer approval, and a canonical baseline series key.

- [x] **5.2 (P0, M)** Wire the four logical quality jobs as required checks while
  preserving existing checks and runtime behavior.
  **Acceptance:** Coverage, contracts, baseline, and documentation failures are
  independently visible; artifacts upload on both success and failure. The
  active organization ruleset `21098892` includes `~DEFAULT_BRANCH`, requires
  `Required Quality Aggregate`, and sets `strict_required_status_checks_policy:
  true`; CI run [33466540997](https://github.com/musutrade/Harness-Gate/actions/runs/33466540997)
  passed the aggregate and all four dependencies.

- [x] **5.3 (P1, S)** Add scheduled and manual baseline refresh workflow rules.
  **Acceptance:** Refreshes produce a reviewable change, never rewrite the
  canonical baseline silently, and retain history and raw samples.

- [x] **5.4 (P1, S)** Define and document the exception process.
  **Acceptance:** An exception requires issue, owner, rationale, expiry, and
  approval; it cannot lower or erase the threshold.

## 6. Verification and Closeout

- [x] **6.1 (P0, S)** Run local and CI-parity validation for all four evidence
  classes.
  **Acceptance:** Commands, artifact schemas, and gate outcomes agree locally
  and in CI; incomplete evidence fails closed. Local evidence was regenerated
  with `coverage.py`, `critical_paths.py`, `contracts.py`, `benchmarks.py`,
  and `docs_consistency.py`; the green CI aggregate is recorded in run
  [33348751556](https://github.com/musutrade/Harness-Gate/actions/runs/33348751556).

- [x] **6.2 (P0, S)** Verify cross-platform structured contracts on Ubuntu,
  macOS, and Windows.
  **Acceptance:** Exit status, error codes, relative paths, report shape, and
  ANSI policy match the declared matrix; differences are documented.

- [x] **6.3 (P0, S)** Review all evidence diffs and update related ADR/OpenSpec
  status only after acceptance.
  **Acceptance:** ADR-0025 links the accepted evidence package, this change is
  marked implemented after the required-check evidence is accepted, and no
  unchecked task is claimed complete without its artifact. The effective rules
  for `main`, the organization ruleset, and the latest green aggregate are
  recorded; an optional failing-PR exercise is not inferred from the green run.

## Evidence Review

- First accepted package: [Phase 1 baseline record](../../../docs/benchmarks/phase-1/README.md)
- Green implementation CI: [PR #33](https://github.com/musutrade/Harness-Gate/pull/33), closeout [PR #39](https://github.com/musutrade/Harness-Gate/pull/39), and [run 33223804928](https://github.com/musutrade/Harness-Gate/actions/runs/33223804928)
- Cross-platform structured contracts: `quality-contracts-33223804928` artifacts from the linked run
- Required-check governance: organization ruleset [`21098892`](https://github.com/musutrade/Harness-Gate/rules/21098892) is active, includes `~DEFAULT_BRANCH`, requires `Required Quality Aggregate`, and sets `strict_required_status_checks_policy: true`.
- Effective rules for `main`: [rules-for-main API](https://api.github.com/repos/musutrade/Harness-Gate/rules/branches/main) returns the required status-check rule.
- Latest accepted `main` run: [33466540997](https://github.com/musutrade/Harness-Gate/actions/runs/33466540997), with all quality jobs and the aggregate successful for commit `6e17867cf42139a3ecd73d014dc566d21d08d562`.
