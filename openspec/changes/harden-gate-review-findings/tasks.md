# Tasks: Harden Gate Boundaries and Delivery Contracts

**Parent:** [proposal.md](proposal.md), [design.md], and
[ADR-0031](../../../docs/adr/0031-harden-gate-boundaries.md)
**Status:** Implemented; follow-up PR #52 and CI run `33311791454` passed on
2026-08-30.

- [x] **1.1 (P0, S)** Add repository-boundary checks for working-tree secret and audit scans.
  **Acceptance:** outside symlink targets are never read; regression tests pass.
- [x] **1.2 (P0, M)** Implement cross-platform process-tree cancellation and built-in scan cancellation points.
  **Acceptance:** descendants terminate on timeout/interrupt; platform tests pass.
- [x] **1.3 (P1, S)** Fix audit rule identity, scope/allowlist compilation, and parser error handling.
  **Acceptance:** collision and performance regressions have focused tests.
- [x] **1.4 (P1, S)** Fix Doctor path resolution and service cleanup diagnostics.
  **Acceptance:** repository-relative checks work from subdirectories and cleanup failures are observable.
- [x] **1.5 (P1, S)** Synchronize CLI snapshots and user-facing installation/remediation text.
  **Acceptance:** contract, docs, and version checks pass.
- [x] **1.6 (P0, S)** Run full validation, update this task list, and verify required CI checks.
  **Acceptance:** all required PR checks are green.

## 2026-08-30 Rust-audit follow-up

- [x] **2.1 (P0, M)** Propagate service cleanup failures, observe cancellation during warmup, and convert worker panics into scheduler failures.
  **Acceptance:** failures remain visible in reports and focused lifecycle tests pass.
- [x] **2.2 (P0, M)** Bound scanner memory, stream log extraction, and make preset initialization a rollback-capable batch.
  **Acceptance:** oversize inputs fail closed; batch success, staging failure, commit rollback, and broken-symlink cases have tests.
- [x] **2.3 (P1, S)** Remove production and integration-test Clippy allowances by deleting unused state or narrowing visibility.
  **Acceptance:** no Rust `allow` attributes remain and strict all-target Clippy passes.
- [x] **2.4 (P1, S)** Decouple audit unit tests from the repository's mutable gate configuration and synchronize user documentation.
  **Acceptance:** the full suite passes with a locally modified `.harness-gate/audit.toml`.
- [x] **2.5 (P0, S)** Review pull-request CI evidence for Linux, macOS, and Windows.
  **Acceptance:** every required check is green and ADR-0031 links the follow-up PR and CI run.

  Completed in PR [#52](https://github.com/musutrade/Harness-Gate/pull/52);
  final commit `24aa22c` passed all required checks in CI run
  [33311791454](https://github.com/musutrade/Harness-Gate/actions/runs/33311791454),
  including the Required Quality Aggregate.
