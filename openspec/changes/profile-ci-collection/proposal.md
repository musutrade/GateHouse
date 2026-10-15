# Profile CI collection and validate test scheduling (#61)

## Goals

Explain current PR and push critical paths using retained stage, command and
job evidence; adopt a bounded test scheduling improvement only after complete
coverage/test parity and measured benefit.

## Non-goals

No cache-policy change, old coverage reuse, parallel mutable profile sharing,
platform/test removal, threshold change, risk-series migration, or push benchmark
rewrite. Engineering Policy semantics remain unchanged, including requiredness,
CRAP, coverage, debt, fresh identity-bound evidence and failure aggregation.

## Success Metrics

- Reproducible PR and push timing cohorts with five observational source runs.
- Separate job work, critical latency, advisory lifecycle and dispatch waiting.
- Full test inventory and coverage evidence for any accepted runner change.
- At least five comparable A/B samples, or explicit insufficient-sample limits;
  no fixed hosted speedup promise from uncontrolled local/observational runs.
- Required CI remains successful with failures retained and no weakened gate.

## Risk

Medium: nextest schedules tests differently and isolates processes; comparison
must catch lost tests, coverage or failures. Roll back the CI runner argument if
coverage or performance regresses. Diagnostic-only timing changes are low risk.

See [diagnosis](../../../docs/quality/issue-61/README.md), the existing
[CI topology proposal](../optimize-ci-execution-topology/proposal.md), and
[engineering policy](../../../docs/engineering-policy.md).
