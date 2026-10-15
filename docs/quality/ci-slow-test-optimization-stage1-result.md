# Slow-test optimization: validated first stage

Local measurements on 2026-10-15, original baseline 2a552a17b894dfa9282fc102d09a1b3c50a5d90a. This is a completed stage, not completion of the expanded object-reduction task. No hosted CI or release performance claim; no commit or publication performed.

Retained changes: compact temporary Python JSON transport, streaming Rust duplicate-key checking with legacy error fallback, and test-profile-only sha2 0.10.9 optimization. Application crates retain debug assertions and overflow checks. Persisted report formats remain unchanged.

| Exact test | Baseline median | Candidate median | Reduction |
| --- | ---: | ---: | ---: |
| Differential | 136.916 s | 47.339 s | 65.43% |
| Policy | 118.286 s | 56.768 s | 52.01% |

Each median uses three interleaved runs. All 12 runs passed. Python stages remained approximately 22 and 26 seconds; sampled process-tree RSS was essentially unchanged. Those observations motivate the next stage rather than claiming object reduction is solved.

One complete suite comparison with default nextest concurrency: 182.948 s / 574 passed versus 92.236 s / 576 passed, zero skipped. This is one pair, not a repeated suite benchmark. Compilation is excluded; initial dependency-profile rebuild took about 30 seconds. Fresh LLVM coverage was exported using actual Cargo executable artifacts; both export stderr files were empty. Risk comparison and selected failures were empty, with 83 source inventory entries on both sides.

Evidence: `/mnt/dev-ssd/gatehouse-slow-test-optimization-20261015/final-validation/result.json`, adjacent suite logs, coverage exports and `risk-comparison.json`. Independent reviewer checked these results. The earlier invalid hash trial that accidentally used a stale copied executable remains archived and is excluded. Older RSS samplers missed thread-spawned descendants and are not used for memory conclusions; current sampling sums process RSS and may double-count shared pages.

Next stage: reduce generated objects and repeated processing while preserving every oracle case, negative case and field comparison. A census found 739,331 objects in Policy transport and 326,790 in Differential. Report inputs and expected reports dominate Policy; expected outputs dominate Differential. An earlier post-generation packing experiment measured only packing/parse/reconstruction, and does not settle the value of generation-time sharing. Independent validation and evaluation outputs must retain their separate semantics.
