# Parallel Scheduling Evidence

This record defines the repeatable evidence package for Phase 4. It is
generated locally or in the `quality-baseline` CI job; generated numbers belong
under `target/quality/` and are not treated as source-controlled baselines
until reviewed.

## Fixture

`tools/quality/fixtures/benchmark` is a no-network Git repository with two
independent external steps. The serial run uses one worker and exercises reuse
of a shared environment service. The parallel run opts into
`[execution] parallel = true` with `max_parallel = 2`; its fixture-only setup
removes the shared service so the product's static resource preflight can keep
the two steps independent. Both modes therefore measure real worker
concurrency without requiring Docker or Podman.

Run it from the repository root:

```bash
python3 tools/quality/benchmarks.py \
  --output target/quality/benchmarks.json \
  --samples 5
```

The command records cold and warm test timings, serial and parallel wall time,
per-step durations, scheduler overhead, the configured concurrency limit, the
fixture-observed peak concurrency, service starts/reuses, report paths, and
retained per-sample reports. Each raw sample also includes the fixture's
`parallel-state.json`, proving that all workers exited and showing the observed
peak rather than treating the configured limit as an observation.
The raw report directory is `target/quality/benchmark-runs/` unless
`--raw-dir` is supplied.

## Comparison Rules

Results are compared by median within the same toolchain, target, fixture
version, and cache-state series. The JSON records this as `series_key` and
includes `regression_policy` alongside the comparison. A parallel regression
is actionable when `verification.comparison.regression` is true: its median
wall time is more than 15% slower than the serial median for the same series.
A speedup is evidence, not a universal performance promise. Results from a
different target or fixture version start a new series and must not be compared
numerically with an existing series.

The report's `verification.comparison` section is the machine-readable source
for the serial median, parallel median, and speedup. Review the retained raw
reports together with the JSON and Markdown summary before accepting a new
baseline.

## Compatibility Evidence

`tools/quality/contracts.py` keeps the publication-order and report-shape
snapshot. The scheduler integration tests cover dependency-local failure,
stable plan-order publication, timeout evidence, unique logs, and reusable or
exclusive service leases. These tests are deterministic and do not make
completion order part of the public contract.
