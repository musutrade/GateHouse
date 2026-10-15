# Slow-test diagnosis — 2026-10-15

Task `GateHouse/slow-test-diagnosis-20261015`, epoch 1. Diagnosis complete; no product behavior or test assertions changed. Evidence paths and SHA-256 identities are recorded in `ci-slow-test-diagnosis.json`.

## Exact tests

Existing coverage-instrumented debug test binary at source `2a552a17b894dfa9282fc102d09a1b3c50a5d90a`, warm cache, one exact test at a time. Both tests passed (one passed, zero failed each).

| Test | Total wall seconds | Python seconds | Outside Python seconds | Generated JSON |
| --- | ---: | ---: | ---: | ---: |
| frozen_policy_and_reference_acceptance_match_python | 118.09 | 26.44 | 91.65 | 142,686,741 bytes |
| retained_corpora_and_consolidated_negative_matrix | 137.04 | 21.99 | 115.05 | 104,205,828 bytes |

Process-tree CPU time closely matches wall time: 118.05 and 137.01 seconds. Python accounts for approximately 22.4% and 16.0%; rewriting the Python orchestration cannot remove most of this measured cost.

## Separate Rust phase diagnostic

External driver linked the existing instrumented debug libraries. It retained policy context/run helpers from the measured snapshot, but used public replay comparison instead of the test's private comparator. Policy processed 826 cases without mismatches; differential replay asserted 33 cases and zero mismatches. Inputs were regenerated (142,714,517 and 104,209,188 bytes), so these are diagnostic phase measurements, not an exact decomposition of the test runs above.

| Phase | Policy seconds | Differential seconds |
| --- | ---: | ---: |
| Read JSON file | 0.075 | 0.057 |
| Production strict JSON parse | 33.934 | 23.215 |
| Case loop / replay and comparison | 53.814 | 89.639 |
| Extra plain serde parse, diagnostic only | 5.107 | 2.923 |

Policy loop includes 51.127 seconds running cases, 0.945 projecting reports, and 0.474 comparing results. The extra plain parse is not a production step; do not add it to production timings. Its difference from strict parsing is not a direct isolated timer for duplicate detection or a promised speedup.

`tools/harness-gate/quality-core/json.rs::parse` first creates a serde Value, then calls `unique` over the original text. The duplicate-key walk allocates RawValue children and recursively reparses their text. This identifies concrete repeated scanning/allocation in strict parsing. Disk reading is negligible in this warm-cache experiment. Rule evaluation/replay is the other large measured cost; its internal functions have not yet been separately timed.

## Python profile

The separate policy cProfile run took 280.63 seconds versus 26.44 seconds unprofiled. Its large overhead means profile durations cannot be treated as normal stage costs. Call counts/hotspots identify repeated evidence/domain/metadata validation: about 1.78 million `_json_domain` calls and 43.51 million `ord` calls, with string validation generators prominent. Profiling supports investigating repeated validation, but does not prove a safe cache or quantify its benefit.

## Next optimization priorities

1. Trial a bounded change reducing repeated work in strict Rust JSON parsing. Preserve nested duplicate-key rejection, arbitrary-precision integers, recursion limits, and error behavior. Compare on identical inputs and instrumented build settings before claiming improvement.
2. Profile inside policy execution and differential replay to distinguish validation, conversion, canonicalization, and rule evaluation. Do not remove validation or cache across changing inputs without establishing equivalent semantics.
3. Investigate Python's repeated domain/metadata checks after measuring the Rust changes; retain independent reference-oracle behavior.

The program is not inherently restricted to Python. Current evidence favors reducing repeated work in the existing mixed Python/Rust implementation over a language rewrite. These are local single measurements, not release-mode or hosted-CI benchmarks; they do not establish a full-suite speedup. Original logs, failed diagnostic build logs, profiles, and generated inputs remain under `/mnt/dev-ssd/gatehouse-slow-tests-20261015/`.
