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
Fresh required same-selection collection and hosted checks remain pending.

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
CI benchmark work. Fresh required quality and hosted validation remain pending.

This document does not claim that staged root relocation, remote sharing,
Windows cache storage or output restoration has been certified.
