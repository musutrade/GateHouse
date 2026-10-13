# Tasks: Strict Standard Result XML

**Parent:** [proposal.md](proposal.md), [design.md](design.md), and
[standard result XML specification](specs/standard-result-xml/spec.md)

- [x] **1.1 (P2, S)** Enforce one recognized JUnit/TRX root and reject content
  outside it.
  **Acceptance:** Empty, wrong-root, multi-root, and trailing-content fixtures
  return errors before their counts are used.
- [x] **1.2 (P2, S)** Preserve namespace-prefixed standard result support.
  **Acceptance:** Qualified JUnit suite/testcase names are classified by local
  name while start/end matching remains exact.
- [x] **1.3 (P2, S)** Add focused accepted/rejected parser regression tests.
  **Acceptance:** All parser tests pass serially.
- [x] **2.1 (P2, S)** Update parser documentation and current follow-up status.
  **Acceptance:** Documentation states the allowed roots and fail-closed cases.
- [x] **2.2 (P2, S)** Run complete local verification.
  **Acceptance:** Format, Clippy, locked tests, strict OpenSpec validation,
  and documentation consistency pass locally. Required CI remains the PR
  closeout check.
- [x] **2.3 (P2, S)** Record green required pull-request CI.
  **Acceptance:** PR #74 required quality aggregate run 33617271136 and all
  required jobs passed; the change status advances to `implemented`.

## Acceptance Evidence

PR [#74](https://github.com/musutrade/Harness-Gate/pull/74) passed required
CI run [33617271136](https://github.com/musutrade/Harness-Gate/actions/runs/33617271136),
including the cross-platform tests/builds, strict Clippy, security audit,
coverage, quality contract checks, performance baselines, documentation
consistency, and `Required Quality Aggregate`.
