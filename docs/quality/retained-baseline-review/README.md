# Retained baseline candidate review (GateHouse #2)

Reviewed on 2026-10-13. **Decision: reject adoption of the complete candidate.**
This resolves [GateHouse #2](https://github.com/musutrade/GateHouse/issues/2).
The accepted [Phase 1 baseline](../../benchmarks/phase-1/README.md) remains unchanged.
The [machine review](review.json) records the original identities and checks.

## Evidence reviewed

The candidate is [Harness-Gate PR #346](https://github.com/musutrade/Harness-Gate/pull/346),
head `f93053a182c07a1c7cc977bb79eed879fb212b8b`, measuring
`4ca947b59d0e85a712fb2eb9e2aa118c207fd67c`. Its successful scheduled capture is
[run 38238245080](https://github.com/musutrade/Harness-Gate/actions/runs/38238245080),
attempt 1, original artifact 11743110115.

The preserved [raw ZIP](https://github.com/musutrade/Harness-Gate/releases/download/baseline-candidate-346-archive-20261013/baseline-candidate-pr346-raw.zip)
is 108,100 bytes and hashes to
`60b6d0502c9b0d5ed8e91ef54923535dd11fbd5aa668c6b266d867531feea7ca`,
matching the issue, original PR and archived
[provenance](https://github.com/musutrade/Harness-Gate/releases/download/baseline-candidate-346-archive-20261013/baseline-candidate-pr346-provenance.json).
Its 195 regular files passed the existing `baseline_delivery.archive_files`
validation. `baseline_delivery.raw_evidence` verified all ten serial/parallel
sample reports, command results, step logs, concurrency records and all five
scope samples against the candidate summary. Recomputed sample medians and the
candidate Markdown figures agree.

## Why adoption is rejected

| Measurement | Accepted record | Candidate |
| --- | --- | --- |
| Warm test median | 116.823 s | 98.072 s |
| Timed test selection | Unfiltered locked nextest suite | Excludes `benchmark_fixture_test` |
| Serial verification median | 0.769 s | 0.763 s |
| Parallel verification median | 0.451 s | 0.447 s |
| Release-small binary | 10,249,672 bytes | 10,321,744 bytes |

The harness at accepted commit
`16873ca9c5ebbd06c6b06d4391ab098fafc05194` times the unfiltered nextest suite.
The candidate's measured source adds
`--filter-expr 'not binary(benchmark_fixture_test)'` and records that exclusion
in `tests_seconds.excluded_binaries`. Both summaries nevertheless retain the
same `harness_version=1`, `fixture_version=1` and series key. The 16.05% lower
warm median therefore mixes two different timed workloads. This does not
establish a same-series performance improvement. Excluding a nested benchmark
can be a valid measurement design change, but needs an explicit reviewed series
transition rather than silent replacement of the old record.

Successful nextest command/stdout/stderr records are absent from this retained
ZIP; the current delivery validator also explicitly reports warm command
records as unauthenticated in this format. The capture succeeded, but its five
warm durations cannot be independently reconstructed from this archive. The
serial/parallel and scope raw evidence is internally consistent; that does not
remove the timed-suite comparability limit or prove equivalence to today's Core.
The binary increase is 0.70%; it is recorded, not treated as the rejection cause.

## Disposition and future updates

No candidate JSON/Markdown replaces the accepted baseline. No measurement is
relabeled as GateHouse evidence, and no threshold, debt, lineage or required gate
is reset. The original PR and preserved archive remain historical evidence.
A future candidate must measure its actual source, declare the timed workload
and measurement-series transition, preserve reviewable raw records and receive
separate baseline adoption review. No new capture is needed to reject this
candidate, and this review does not authorize a new baseline.

Engineering Policy semantics are unchanged. This review applies to the Phase 1
performance candidate, not to adoption of coverage/CRAP debt or trusted project
baseline bundles.
