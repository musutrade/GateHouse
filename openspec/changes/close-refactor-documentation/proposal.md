# Proposal: Close Refactor Documentation

**Status:** Implemented

## Why

Merged module decompositions still have OpenSpec records that report pending
implementation PRs or CI. This makes the completed refactor programme appear
incomplete and hides the separate post-merge monitoring work that remains.

## Goals

- Record merged refactors as complete after `main` CI has passed.
- Accept ADR-0020 now that its implementation has merged.
- Preserve the three Phase 2 observation tasks as incomplete until their
  time-bound acceptance criteria are met; they were subsequently completed
  after the observation window in PR #57.

## What Changes

- Add ADR-0021 and its index entry.
- Update the status lines and completed checklists for merged refactor changes.
- Add this OpenSpec closeout record.

## Non-goals

- No production Rust, configuration, workflow, dependency, or test changes.
- No completion claim for post-merge monitoring that has not occurred.

## Risk Assessment

Low: the change is documentation-only and does not alter runtime behavior.

## Success Metrics

- Every merged refactor record reports merged status and `main` CI success.
- ADR-0020 is Accepted.
- Phase 2 tasks 9.1 through 9.3 are not completed before their observation
  criteria; their later completion retains the supporting evidence.
- Strict OpenSpec validation passes.

## Implementation Evidence

- Documentation closeout: [PR #28](https://github.com/musutrade/Harness-Gate/pull/28)
- Task-record follow-up: [PR #32](https://github.com/musutrade/Harness-Gate/pull/32)
- Accepted CI evidence: [run 33060206365](https://github.com/musutrade/Harness-Gate/actions/runs/33060206365)
- Phase 2 observation closeout: [PR #57](https://github.com/musutrade/Harness-Gate/pull/57)

## Related

- [ADR-0020](../../../docs/adr/0020-audit-scanner-decomposition.md)
- [Phase 2 monitoring tasks](../implement-phase-2-optimization/tasks.md)
