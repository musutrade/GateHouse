# Tasks: Post-Remediation Security, Reliability, and Maintenance Hardening

**Parent:** [proposal.md](proposal.md), [design.md](design.md), and
[post-remediation hardening specification](specs/post-remediation-hardening/spec.md)

**Status:** Implemented (2026-09-03). All R-07 and R-13 through R-18 tasks
are closed with focused tests, benchmark/allocation evidence, documentation
consistency fixtures, and Linux/macOS/Windows CI evidence. A future
OS-sandbox implementation remains a separate non-goal tracked by
[`os-sandbox-decision-matrix.md`](../../../docs/os-sandbox-decision-matrix.md);
DevRail staging G-03/G-04, shadow/canary, and rollback authority remain
external governance work.

Every implementation task is intended to fit within four hours. A task may be
checked only after its focused tests, compatibility notes, and acceptance
evidence are reviewed.

## 1. R-07 Isolation Contract and Future Sandbox Boundary

- [x] **1.1 (P1, S)** Inventory adapter isolation wording across README,
  configuration docs, ADRs, OpenSpec, and CLI help.
  **Acceptance:** Every current claim is classified as protocol, lifecycle, or
  evidence control; no current text claims OS-enforced sandboxing.
- [x] **1.2 (P1, S)** Write the separate OS-sandbox decision matrix for Linux,
  macOS, and Windows without implementing a sandbox in this change.
  **Acceptance:** The matrix names platform primitives, identity, network,
  filesystem, resource, descendant, and unavailable-feature behavior.
- [x] **1.3 (P1, S)** Add documentation-consistency fixtures for the bounded
  wording and future-sandbox boundary.
  **Acceptance:** The check fails on an unsupported sandbox claim and passes on
  the current R-07 wording.

## 2. R-13 Webhook Egress Policy

- [x] **2.1 (P2, S)** Approve the URL, host-allowlist, and address-classification
  contract.
  **Acceptance:** Public, loopback, RFC1918/private, link-local, unspecified,
  multicast, and malformed-host cases have deterministic outcomes.
- [x] **2.2 (P2, M)** Implement connection-time resolution and address
  re-checking with explicit redirect behavior.
  **Acceptance:** A DNS-rebinding fixture and every denied address class fail
  closed before connection; no policy decision logs credentials or body data.
- [x] **2.3 (P2, S)** Add allowlist, redaction, and cross-platform webhook
  regression tests.
  **Acceptance:** An allowlisted public destination succeeds in the test
  double; denied destinations produce one structured failure and no request.

## 3. R-14 Cross-Platform Lease Liveness

- [x] **3.1 (P2, S)** Specify platform identity and lease-renewal timing for the
  complete allocation-to-cleanup lifecycle.
  **Acceptance:** Linux, macOS, and Windows behavior records the identity source,
  renewal deadline, and uncertain-identity outcome.
- [x] **3.2 (P2, M)** Implement a host-owned heartbeat or equivalent renewal
  mechanism and structured ownership-uncertain failures.
  **Acceptance:** A step longer than the TTL remains owned; renewal failure or
  identity reuse performs zero destructive removals.
- [x] **3.3 (P2, M)** Add lifecycle, restart, cancellation, and cross-platform
  lease tests.
  **Acceptance:** Allocation, execution, cleanup, and failure evidence cover
  the same immutable identity on every supported CI platform.

## 4. R-15 Typed Failure and Diagnostic Contracts

- [x] **4.1 (P2, S)** Approve the `FailureCode` registry, retry classes,
  serialized spelling, and compatibility policy.
  **Acceptance:** Existing machine consumers have a migration table and no
  code is assigned from human wording.
- [x] **4.2 (P2, M)** Introduce structured runtime and configuration diagnostics
  while preserving safe human rendering.
  **Acceptance:** Code, severity, path, retry class, help, and related context
  are available without resolved secret values.
- [x] **4.3 (P2, M)** Migrate scheduler, retry, report, and CLI branches from
  display-string inference.
  **Acceptance:** A wording-only snapshot change cannot change machine behavior;
  unknown producer codes fail closed at contract boundaries.

## 5. R-16 Polling and Scheduler Performance

- [x] **5.1 (P3, S)** Record reproducible baseline benchmarks for fast/slow
  commands, cancellation, failed nodes, and narrow/wide/deep DAGs.
  **Acceptance:** The benchmark environment, command, dataset, and variance
  are recorded under `docs/benchmarks`.
- [x] **5.2 (P3, M)** Replace fixed polling with a wait primitive or bounded
  exponential backoff that observes deadlines and cancellation.
  **Acceptance:** Fast commands avoid the fixed delay; timeout, cancellation,
  and evidence behavior remain unchanged.
- [x] **5.3 (P3, M)** Add deterministic scheduler indexes and remaining-
  dependency tracking.
  **Acceptance:** Only affected dependents are reconsidered, resource-preflight
  exclusions remain enforced, and ordering snapshots remain stable.
- [x] **5.4 (P3, S)** Compare post-change benchmarks and publish the result.
  **Acceptance:** Improvement or non-improvement is reproducible and documented;
  no performance claim relies on a single unrepeatable run.

## 6. R-17 Configuration and Diagnostic Maintainability

- [x] **6.1 (P2, M)** Replace per-item whole-config cloning with borrowed
  validation context or focused sub-contexts.
  **Acceptance:** Validation semantics and source locations remain unchanged;
  representative large configurations show bounded allocation behavior.
- [x] **6.2 (P2, M)** Convert documented closed vocabularies to typed enums and
  define the environment-variable namespace migration.
  **Acceptance:** Unknown values fail before external work; aliases have explicit
  warnings, expiry, and compatibility tests.
- [x] **6.3 (P2, S)** Document and test release-profile panic behavior,
  `catch_unwind`, and lock poisoning.
  **Acceptance:** Linux and applicable cross-platform checks exercise the chosen
  failure behavior without relying on implicit profile assumptions.

## 7. R-18 Repository, Community, and Release Metadata

- [x] **7.1 (P2, S)** Remove or archive one-off artifacts and add ignore rules
  for `__pycache__/` and `*.pyc`.
  **Acceptance:** No cache artifacts are tracked and the cleanup list is recorded.
- [x] **7.2 (P2, S)** Add `SECURITY.md`, MSRV metadata, Issue/PR templates, and
  any required behavior/contributor guidance.
  **Acceptance:** Links, ownership, supported-version policy, and reporting
  paths pass documentation consistency checks.
- [x] **7.3 (P2, M)** Put quality scripts under lint/test and correct the secret
  scan documentation.
  **Acceptance:** CI validates the scripts and the docs state content scanning
  is a quick preflight, not a replacement for dedicated scanners.

## 8. Verification and Closeout

### Acceptance Evidence

The selected implementation tracks are merged to `main` as part of v0.3.6
([PR #75](https://github.com/musutrade/Harness-Gate/pull/75), commit
`6a9066b7f5dba241a3190a5508727cd21ba2c9b0`). Focused tests cover webhook
egress and DNS-rebinding, lease heartbeat and platform identity, typed
failure contracts, scheduler behavior, panic/lock-poisoning, and bounded
adapter-isolation wording.

- R-07 inventory: [`adapter-isolation-wording-inventory.md`](../../../docs/adapter-isolation-wording-inventory.md)
- R-07 decision matrix: [`os-sandbox-decision-matrix.md`](../../../docs/os-sandbox-decision-matrix.md)
- R-07 consistency fixtures: `unsupported_sandbox_claims` in
  [`docs_consistency.py`](../../../tools/quality/docs_consistency.py) and
  [`test_docs_consistency.py`](../../../tools/quality/tests/test_docs_consistency.py)
- R-16 scenario comparison:
  [`r16-scenarios.md`](../../../docs/benchmarks/r16-r17-evidence-2026-09-03/r16-scenarios.md)
  with per-sample JSON
- R-17 allocation evidence:
  [`r17-config-allocation.md`](../../../docs/benchmarks/r16-r17-evidence-2026-09-03/r17-config-allocation.md)

Local closeout on 2026-09-03: `cargo nextest run --locked` passed 290/290,
format and strict Clippy passed, documentation consistency passed, and the
quality script suite passed 7/7. PR and protected `main` CI runs
[33701746324](https://github.com/musutrade/Harness-Gate/actions/runs/33701746324),
[33702793530](https://github.com/musutrade/Harness-Gate/actions/runs/33702793530),
and the `Required Quality Aggregate`
[job 100488162319](https://github.com/musutrade/Harness-Gate/actions/runs/33702793530/job/100488162319)
provide the Linux/macOS/Windows evidence. A future OS-sandbox implementation
is intentionally outside this umbrella; DevRail staging G-03/G-04,
shadow/canary, and rollback authority are external governance acceptance.

- [x] **8.1 (P1, M)** Run focused tests and review failure-path evidence for
  every implemented track.
  **Acceptance:** Each track has a linked regression fixture, compatibility
  note, and rollback procedure.
- [x] **8.2 (P1, M)** Run locked tests, formatter, strict Clippy, audit,
  documentation consistency, and applicable Linux/macOS/Windows CI checks.
  **Acceptance:** All required checks are green, or the limitation and owner
  are recorded without weakening the contract.
- [x] **8.3 (P1, S)** Update this OpenSpec and ADR status only after all selected
  implementation evidence is accepted.
  **Acceptance:** No incomplete task is checked and the umbrella status remains
  Proposed while any R-07/R-13 through R-18 track is open.
