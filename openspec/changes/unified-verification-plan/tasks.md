# Tasks: Unified Verification Plan and Gate Execution Contract

**Parent:** [proposal.md](proposal.md), [design.md](design.md), and
[verification-plan specification](specs/verification-plan/spec.md)
**Status:** Implemented; acceptance evidence reviewed against PR #35 and the
green main CI run `33167763113`.
**Implementation boundary:** This change implements only the private plan and
compatibility behavior described by the parent records. It adds no scheduler,
new business gate, or report-schema migration.

Every task is intentionally smaller than four hours and must retain the
existing CLI, report, error-code, configuration, and runtime contracts unless a
separate ADR explicitly approves a change.

## 1. Contract Inventory

- [x] **1.1 (P0, S)** Inventory current fixed gate orchestration.
  **Acceptance:** The inventory records gate order, selected-step closure,
  profile/scope behavior, output streams, report fields/paths, and all
  `E1402`/`E1403`/`E1404` cases with fixture references.

- [x] **1.2 (P0, S)** Approve the private `VerificationPlan`, `PlanNode`, and
  `NodeResult` field registry.
  **Acceptance:** IDs, kinds, statuses, dependency semantics, selection state,
  adapter ownership, and privacy boundary are reviewed and versioned.

- [x] **1.3 (P0, S)** Approve the built-in `kind`/`gate_type` registry.
  **Acceptance:** `secret-scan` and `architecture-audit` are the only accepted
  values; unknown values, field combinations, reserved IDs, and migration
  behavior have explicit decisions.

## 2. Default DAG and Selection Contract

- [x] **2.1 (P0, M)** Specify the synthesized legacy DAG matrix.
  **Acceptance:** Fixtures cover empty/direct/transitive dependencies,
  `step <id>`, profile selection, scope selection, and stable ties; expected
  node IDs and order are reviewable.

- [x] **2.2 (P0, M)** Specify explicit built-in-node validation.
  **Acceptance:** Valid declarations, duplicate IDs, unknown gate types,
  missing references, bypass attempts, and cycles have expected diagnostics and
  no side effects.

- [x] **2.3 (P1, S)** Define scheduler handoff invariants.
  **Acceptance:** A future scheduler must consume the same dependency closure,
  ADR-0026 resource relations, mandatory gate chain, and cancellation
  invariants; any relaxation requires a new ADR/OpenSpec.

## 3. Unified Result, Failure, and Cancellation Contract

- [x] **3.1 (P0, M)** Define the adapter-to-`NodeResult` mapping.
  **Acceptance:** Secret, audit, external success/failure, timeout, skipped,
  cancellation, service setup, parser, and report-write outcomes map to status,
  detail, artifacts, and public error codes without secret disclosure.

- [x] **3.2 (P0, M)** Define prerequisite propagation and unrelated-step
  continuation.
  **Acceptance:** Gate failures, descendant blocking, unrelated external-step
  continuation, and overall pass computation match captured current behavior.

- [x] **3.3 (P0, M)** Define cancellation and cleanup evidence.
  **Acceptance:** Before-node cancellation, running-process cancellation,
  descendant status, service/process cleanup, timeout, and `E1402` are covered
  by actual boundary fixtures on applicable platforms.

## 4. Compatibility Evidence Plan

- [x] **4.1 (P0, M)** Prepare fixed-orchestration reference fixtures.
  **Acceptance:** The reference captures node order, output streams, reports,
  report paths, logs, exit status, error codes, and cleanup observations.

- [x] **4.2 (P0, M)** Prepare synthesized-DAG equivalence fixtures.
  **Acceptance:** The same inputs produce equivalent observable behavior for
  success, secret failure, audit failure, external failure, cancellation,
  timeout, parser failure, and report-write failure.

- [x] **4.3 (P1, S)** Add deterministic plan and result snapshots.
  **Acceptance:** Snapshots normalize only approved dynamic values; node order,
  statuses, labels, paths, report shape, and error-code forms remain literal.

## 5. Documentation and Review Closeout

- [x] **5.1 (P1, S)** Document the configuration discriminator and migration
  boundary.
  **Acceptance:** Existing files remain valid when `kind` is absent; explicit
  built-in declarations, unknown types, and non-silent repairs are documented.

- [x] **5.2 (P0, S)** Review the implementation diff for scope compliance.
  **Acceptance:** No scheduler, renderer, new business gate, service lock,
  retry, or unrelated report/configuration behavior is present.

- [x] **5.3 (P0, S)** Update ADR/OpenSpec status only after implementation
  evidence is accepted.
  **Acceptance:** ADR-0027, this change, compatibility artifacts, and any
  follow-up scheduler/gate proposals are linked; no unchecked task is claimed
  complete.

## Evidence Review

- Implementation: [PR #35](https://github.com/musutrade/Harness-Gate/pull/35)
- Green cross-platform and quality CI: [run 33167763113](https://github.com/musutrade/Harness-Gate/actions/runs/33167763113)
- Compatibility evidence: `tools/harness-gate/src/verify/tests.rs`, `tools/quality/contracts.py`, and `tools/quality/snapshots/contracts.json`
