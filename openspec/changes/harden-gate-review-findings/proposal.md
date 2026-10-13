# Proposal: Harden Gate Boundaries and Delivery Contracts

**Status:** Implemented

## Goals

- Keep secret and audit scans inside the selected repository, including when
  the working tree contains file symlinks.
- Make cancellation terminate the complete external process tree on every
  supported platform and allow built-in scans to stop promptly.
- Restore green, reproducible quality gates and make user-facing commands and
  installation instructions match the `harness-gate` binary.
- Remove correctness and avoidable performance regressions found in audit,
  scope, doctor, parser, and service paths.

## Non-goals

- Do not introduce a public plugin API or replace the Git-based scope model.
- Do not change the report schema or the configured gate ordering.
- Do not include unrelated working-tree documentation, scripts, or local
  configuration changes.

## Success Metrics

| Area | Success criterion |
| --- | --- |
| Quality | CLI contract snapshot, formatter, linter, tests, and docs checks pass. |
| Boundary | Symlink files outside the repository are skipped or rejected by both scanners. |
| Cancellation | Windows process descendants are contained and built-in scans observe cancellation. |
| Correctness | Audit rule counts are keyed by rule identity rather than string prefixes. |
| Performance | Scope globs and audit allowlist regexes are compiled once per configuration. |
| UX | Install/remediation examples invoke `harness-gate` and point to the real repository. |

## Risk Assessment

Medium. Process lifecycle and scanner boundary changes affect every verification
run; focused regression tests and platform-gated implementations limit rollout
risk. Documentation and snapshot updates are low risk.

## Related Records

- [ADR-0031](../../../docs/adr/0031-harden-gate-boundaries.md)
- [ADR-0025](../../../docs/adr/0025-phase-1-quality-baseline-gates.md)
- [ADR-0028](../../../docs/adr/0028-parallel-scheduling.md)
