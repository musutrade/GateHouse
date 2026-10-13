# Tasks: Dependency-Aware Parallel Scheduling

**Parent:** [proposal.md](proposal.md), [design.md](design.md), and
[parallel-scheduling specification](specs/parallel-scheduling/spec.md)
**Status:** Implemented; acceptance evidence reviewed against PR #36 and the
green main CI run `33177851104`.
**Implementation restriction:** Tasks describe the reviewed delivery boundary;
they must not add unrelated business behavior, gates, report fields, or public
APIs.

Every task is bounded to less than four hours and has an explicit acceptance
criterion. Tasks may be marked complete only after the linked evidence is
reviewed.

## 1. Contract Inventory and Policy

- [x] **1.1 (P0, S)** Capture the pre-scheduler serial execution contract.
  **Acceptance:** Fixtures record gate/step order, scope/profile selection,
  unrelated-step failure continuation, output streams, report paths, logs,
  cleanup state, and `E1402`/`E1403`/`E1404` results.

- [x] **1.2 (P0, S)** Approve the `[execution]` schema and diagnostics.
  **Acceptance:** Omitted/false/true cases, default 4, range 1–64, zero,
  negative, and over-limit values have deterministic schema and config-check
  outcomes before any side effect.

- [x] **1.3 (P0, S)** Define scheduler state and policy registry.
  **Acceptance:** Node states, dependency-local failure behavior, optional
  plan-level fail-fast cancellation, timeout reason, and primary-error
  selection are versioned without exposing a public scheduler API.

## 2. Scheduler and Plan Integration

- [x] **2.1 (P0, M)** Specify the scheduler input/output boundary.
  **Acceptance:** The boundary consumes the ADR-0027 plan and returns ordered
  `NodeResult` values; no second dependency ordering algorithm is introduced.

- [x] **2.2 (P0, M)** Define deterministic ready-queue dispatch.
  **Acceptance:** Unit fixtures prove dependency readiness, stable ties,
  serial mode, parallel limits, and no dispatch after cancellation.

- [x] **2.3 (P0, M)** Define descendant blocking and unrelated-branch behavior.
  **Acceptance:** Failed, cancelled, skipped, and timed-out ancestors produce
  the exact documented statuses; independent branches follow the captured
  policy in both serial and parallel modes.

- [x] **2.4 (P1, S)** Verify built-in gate compatibility.
  **Acceptance:** Synthesized secret/audit edges remain mandatory and public
  gate labels, paths, output, and failure codes match ADR-0027 fixtures.

## 3. Services, Logs, and Publication

- [x] **3.1 (P0, M)** Define adapter resource identity and shareability.
  **Acceptance:** Reusable and exclusive service cases specify lock mode,
  startup reuse, bounded wait, failed-start rollback, and exactly-once cleanup.

- [x] **3.2 (P0, M)** Define task-local execution context and environment rules.
  **Acceptance:** Tests or contract fixtures demonstrate no process-global
  environment mutation and no unsynchronized mutable adapter state.

- [x] **3.3 (P0, M)** Define unique log allocation and containment.
  **Acceptance:** Normalization collisions, duplicate names, per-step paths,
  report-root containment, and no-overwrite behavior have expected outcomes.

- [x] **3.4 (P0, M)** Define the ordered result publisher.
  **Acceptance:** Controlled completion reordering yields identical normalized
  CLI/report output, stable primary error selection, and existing report shape.

## 4. Cancellation and Failure Evidence

- [x] **4.1 (P0, M)** Specify queued, lock-waiting, and running cancellation.
  **Acceptance:** Each state has a terminal status, cleanup expectation,
  descendant policy, and `E1402` assertion.

- [x] **4.2 (P0, M)** Specify timeout and process/service cleanup evidence.
  **Acceptance:** Timeout retains logs, stops owned work, blocks descendants,
  and leaves no managed process/service/lock leak.

- [x] **4.3 (P0, S)** Specify multi-failure aggregation.
  **Acceptance:** Stable plan order selects the primary `E1403` failure while
  other node failures remain inspectable without changing legacy output.

## 5. Performance, Contention, and Compatibility

- [x] **5.1 (P1, M)** Prepare deterministic serial/parallel benchmark fixtures.
  **Acceptance:** A no-network/no-Docker fixture records wall time, per-step
  duration, peak concurrency, scheduler overhead, service-start reuse, target,
  toolchain, cache state, and fixture metadata.

- [x] **5.2 (P1, M)** Prepare service-contention and repeated-run fixtures.
  **Acceptance:** Repeated runs cover shared reusable services, exclusive
  resources, lock cancellation, startup failure, cleanup, and no artifact
  leakage.

- [x] **5.3 (P0, M)** Prepare CLI/report/error compatibility snapshots.
  **Acceptance:** Serial and parallel fixtures assert stable publication order,
  report paths/names/shape, ANSI policy, and `E1402`/`E1403`/`E1404` mappings.

- [x] **5.4 (P1, S)** Define benchmark comparison and regression governance.
  **Acceptance:** ADR-0025-compatible series keys, sampling, 15% regression
  handling, and incompatible-target baseline behavior are documented.

## 6. Documentation and Closeout

- [x] **6.1 (P1, S)** Update configuration and migration documentation.
  **Acceptance:** Docs explain serial default, default/range of `max_parallel`,
  log naming, resource constraints, cancellation, and stable output order.

- [x] **6.2 (P0, S)** Run specification and evidence review.
  **Acceptance:** `openspec validate parallel-scheduling --strict
  --no-interactive` passes; all required scenarios have owners and fixture
  links; no implementation task claims completion without evidence.

- [x] **6.3 (P0, S)** Update ADR/OpenSpec status after implementation only.
  **Acceptance:** ADR-0028 and this change link the implementation PR,
  benchmark artifacts, compatibility snapshots, and any approved deviations;
  proposed status remains until the green CI evidence is reviewed.

## Evidence Review

- Implementation: [PR #36](https://github.com/musutrade/Harness-Gate/pull/36)
- Green cross-platform and quality CI: [run 33177851104](https://github.com/musutrade/Harness-Gate/actions/runs/33177851104)
- Benchmark and compatibility records: [parallel scheduling benchmark](../../../docs/benchmarks/parallel-scheduling.md) and the quality runners under `tools/quality/`
