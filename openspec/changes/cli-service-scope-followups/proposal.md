# Proposal: CLI, Service Overlap, and Scope Evidence Follow-ups

The remaining review follow-ups are user-facing diagnostics, verification
startup latency, and evidence quality. This change keeps the existing schema and
gate order while making those concerns observable and measurable.

## Goals

- Make failed human `config check` output directly actionable and document the
  common CLI workflows in English and Chinese.
- Prewarm only services selected by the verification plan while secret scan and
  architecture audit run, without holding an execution lease.
- Add an in-process cached-versus-uncached scope matcher benchmark to the
  quality baseline.

## Non-goals

- No new service provider or change to gate ordering.
- No change to the public JSON report schema or default scope behavior.
- No benchmark comparison across different targets, toolchains, or fixtures.

## Risks and Mitigations

- A prewarm failure is cached on the same resource and is surfaced by the real
  step; worker joins ensure no startup thread is detached.
- Benchmark output records equivalence and series metadata so an unstable or
  incomparable sample cannot be treated as a regression.
