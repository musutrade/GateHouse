# Post-Remediation Hardening Benchmark

**Status:** Local candidate evidence (not a release baseline)

This report records the repeatable Linux sample for the R-16 scheduler and
scope-matcher work. It was collected from the current worktree after the
lease-heartbeat test-fixture fix; the recorded source commit is the clean
`main` ancestor and does not include the uncommitted worktree changes.

## Environment

- Command: `python3 tools/quality/benchmarks.py --output target/quality/benchmarks.json --samples 3 --raw-dir target/quality/benchmark-runs`
- OS/target: Linux 7.0.0-30-generic, `x86_64-unknown-linux-gnu`
- Toolchain: `rustc 1.97.1`, `cargo 1.97.1`, Python 3.14.4
- Logical CPUs: 8
- Fixture and harness versions: 1 / 1
- Cache series: cold-and-warm; three samples for each median
- Source metadata: commit `68e936cdbc987703cc54d0f25f4d8615f8249800`

The raw per-sample reports remain local under
`target/quality/benchmark-runs/`; they are not source-controlled evidence.

## Results

| Measurement | Result |
| --- | ---: |
| Serial verification median | 0.9685 s |
| Parallel verification median | 0.6393 s |
| Parallel observed peak | 2 workers |
| Parallel speedup | 1.515x |
| Parallel delta | -33.99% |
| Regression threshold | 15% |
| Regression detected | No |
| Scope matcher cached median | 11,026.48 us/iteration |
| Scope matcher uncached median | 18,921.71 us/iteration |
| Scope matcher speedup | 1.721x |
| Scope matcher equivalence | true over 601 paths and 100 iterations |
| Warm nextest median | 7.448 s |
| Cold nextest sample | 7.527 s |
| Release-small binary | 8,703,856 bytes |

Serial samples were 0.9749 s, 0.9685 s, and 0.9652 s. Parallel samples were
0.6393 s, 0.6386 s, and 0.6396 s. The benchmark fixture exercised two
independent external steps and a shared service in the serial path; no Docker
or Podman runtime was required.

## Boundaries

This is a Linux worktree candidate, not a cross-platform or staging baseline.
It does not provide macOS/Windows CI evidence, DNS-rebinding fixture evidence,
or DevRail G-03/G-04 acceptance. Those remain required before the umbrella ADR
or OpenSpec can leave `Proposed`.
