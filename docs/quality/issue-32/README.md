# Issue #32 validation and measurement

Implementation contract: [local step cache](../../step-cache.md).
Engineering Policy semantics are unchanged. No cached project decision,
required-gate skip, waiver expansion or retained baseline reset is introduced.

The initial complete workspace nextest run passed all 550 tests with zero skips.
After decomposing process waiting/output validation, the binary suite passed all
427 tests. Subsequent cache pinning/identity regressions passed 20 unit tests and
the real CLI integration test covering configuration, read-only preview, a cold
miss, a hit, current mandatory gates, host disable and authenticated clearing.
The latest Clippy run with all targets/features and warnings denied passed.
The initial publication did not claim hosted approval; implementation results are recorded below.

The risk inventory is explicitly expanded to gh32-step-cache/1. Both commits
must be freshly collected with that identical selection and the existing pinned
analyzer/metric definitions. Historical measurements, thresholds and mandatory
critical-path applicability are unchanged. Existing source probes follow the
actual process wait branches and parser/service logic extracted into helpers;
no probe is replaced with a wrapper that cannot observe the failure branch.

The optional [synthetic benchmark](../../../tools/quality/step_cache_benchmark.py)
compiles a fixed deterministic CPU checker and runs alternating uncached/cached
full verifications on Linux. It binds the checker and resolved dynamic libraries,
checks identical logs and complete current reports, and reports cold lookup/store
cost separately. It is not a required CI job or an adopted performance baseline.

```console
python3 tools/quality/step_cache_benchmark.py --binary /path/to/release/harness-gate --output target/issue32/benchmark.json
```

The release binary was measured with seven alternating samples per workload.
The original short-workload regression is retained, not hidden by the longer case:

| Synthetic series | Uncached verify median | Hit verify median | End-to-end change |
| --- | ---: | ---: | ---: |
| [100 million iterations](benchmark-short.json) | 1382.213 ms | 1532.400 ms | **10.87% slower** |
| [1 billion iterations](benchmark-long.json) | 4038.444 ms | 1524.898 ms | **62.24% faster** |

The short command itself took about 319 ms; authenticated hit lookup took about
464 ms. For the longer command those costs were about 2962 ms and 462 ms.
Cold misses additionally pay post-execution input revalidation and publication.
Raw records retain every sample, cold timings, Core/checker/library digests,
compiler and host identity. These are two explicitly separate synthetic series,
not a universal speed claim or an adopted baseline. Existing mandatory built-in
checks and fresh report sealing are included in every end-to-end timing.

Version 1 is appropriate only when project measurements justify its complete
input-scan cost. It stays disabled by default. This experiment adds no required
CI benchmark work. Hosted results are recorded separately below; these timings confer no release approval.

This document does not claim that staged root relocation, remote sharing,
Windows cache storage or output restoration has been certified.

## First hosted review and repairs

[The first hosted run](https://github.com/musutrade/GateHouse/actions/runs/38675458258)
passed Linux/Windows tests, build, Clippy, documentation, release/security checks,
legacy/production coverage and the mandatory critical-path matrix. The run
remains failed: CLI help needed its reviewed additive snapshot, the optional
benchmark needed category-A Python tooling registration, and macOS fixtures
needed canonical temporary paths rather than /var aliases. The directory
no-follow/ownership checks were retained. A Linux run using a deliberately
symlinked TMPDIR passed all 20 cache unit regressions and the real CLI test after
canonicalizing only the fixture paths; the subsequent hosted run below provides real macOS confirmation.

Fresh same-selection risk comparison also rejected three newly changed/extracted
functions for inadequate coverage:

| Function | Line coverage | Region coverage |
| --- | ---: | ---: |
| process/task.rs::wait_for_exit | 83.33% | 77.19% |
| verify/steps.rs::run_uncached_step | 70.91% | 75.71% |
| verify/steps.rs::log | 71.43% | 64.29% |

The repair adds operational negative regressions: both stream overflow signals
terminate live children and remain visible after child exit; a poisoned service
manager blocks dispatch with ServiceSetupFailure; log allocation creates missing
confined directories while rejecting invalid names, outside report roots and
linked directory/file escapes without changing protected targets. Each focused
regression passed locally. The implementation, required 80% line/region floors,
CRAP limit, inventory selection and historical baselines remain unchanged.
A new complete hosted collection was required; its results are below. The failed
run and raw artifacts are retained rather than reclassified as a success.

## Implementation acceptance

[The second hosted run](https://github.com/musutrade/GateHouse/actions/runs/38677572819)
passed Required Quality Aggregate and every required child job, including real
Linux, macOS and Windows tests. Linux nextest ran **559 tests, all passed with zero
skips**. The fresh legacy/production coverage, risk and mandatory critical-path
stages all passed. The optional generic shadow has no release authority.

The tested feature commit is `4d5a435a8f4a4d1e3fff3a29991b8219e874cdc4`;
CI's actual merge snapshot is `1ddf2394c416233e1dea5749cdc8a25019e5c93d` against
`0b4b3ee132235b00f4b3dc2525ab0967519e6901`. Both sides used
`gh32-step-cache/1` and the existing pinned analyzer/definitions. The sealed
`ci-artifact.json` SHA-256 is `9681ee37621280aa8100dd290c02a64b0b83b8d219ea5c491301c6a06f2affd8`.

| Repaired function | Line coverage | Region coverage | CRAP (rounded) |
| --- | ---: | ---: | ---: |
| process/task.rs::wait_for_exit | 100.00% | 91.23% | 14.000000 |
| verify/steps.rs::run_uncached_step | 93.64% | 88.57% | 17.074476 |
| verify/steps.rs::log | 97.62% | 87.50% | 14.002646 |

The final fixture refinement places toolchain data outside the repository. Its
mutation test therefore proves explicit toolchain binding independently of the
implicit source scan; all 20 cache regressions passed again locally. That test
and this acceptance record undergo the normal final-commit CI before review.
These updates do not change production execution, policy, thresholds, baseline
history or the measurement selection. No prior report is adopted as evidence for
a different commit.
