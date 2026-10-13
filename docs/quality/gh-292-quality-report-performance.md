# Quality evaluation and report performance (#292)

The fixed-input Linux experiment compares source commit `e4a084be19de454cdaeebb44c41715fecf1f2b49` with that source plus the four frozen evidence, policy, project-report and text-redaction changes. Both variants use Rust 1.99.0 / LLVM 23.1.1, the same locked release diagnostic package, compiler flags and extracted production JSON helpers. These are retained experiment identities; a later Git head is not relabelled as the measured source.

The small fixture is the existing failing cross-component contracts corpus. The large fixture has 1,000 distinct synthetic subjects and four component policies. Both variants consume the same immutable 1,024 input files; the manifest itself is additionally hashed. External collectors are absent. Each fixture and variant has three fresh-process samples. OS page cache is uncontrolled, so these are not cold-cache results.

| Fixture | Wall median before / after (s) | CPU median before / after (s) | Peak RSS before / after (KiB) | Report bytes |
| --- | --- | --- | --- | --- |
| Small | 0.022603202 / 0.021351519 | 0.022307 / 0.021100 | 7,832 / 7,936 | 119,801 |
| Large | 2.006410478 / 1.462108975 | 1.969364 / 1.426544 | 140,308 / 140,540 | 6,814,167 |

Large wall time decreased 27.13% and CPU time decreased 27.56% in this experiment. The short small-fixture sample ranges overlap. Peak RSS did not improve. The large report digest is `a014d85577735b2340b9804072fbab29d661f248b1ca9f7854036c45178ffe48`; the small digest is `ac7143e070bdb72cf6b6c9fe282877160b5c39c3f047302f1f716ffd63f4a891`, identical before and after.

The large standalone evidence validation phase changed 0.636081020→0.481190770 seconds, policy evaluation including validation 0.915481432→0.591803776, project-report construction 0.083497963→0.066386604, and JSON redaction 0.162536326→0.127917613. These three-sample phase medians support the selected scanning/copying changes, without attributing a production outer-write gain.

Raw phase records separate input parsing, evidence validation, policy evaluation, project report construction, Value conversion, recursive redaction, pretty serialization and writes. Standalone evidence validation repeats work included inside policy evaluation; do not add those times as a production total. Raw CPU and Linux `rchar`, `syscr` and `read_bytes` deltas include diagnostic reads. They respectively describe bytes returned to reads, read syscall count and physical storage I/O charged by the kernel. Zero physical I/O does not mean no validation. Peak RSS is process cumulative highwater. The summaries retain per-phase medians and every sample, including write bytes.

Ten new exact tests, frozen evidence/policy/reference contracts, original strict/redaction regressions and 84 differential public API parity rows preserve Value, ordering, duplicates/ambiguity, fail-closed errors and deterministic report bytes. Full-value series memoization is limited to one evaluation batch; validated subjects compare complete values. There is no file/hash or persistent cache. Regex patterns, replacement order and public String API remain.

## Actual product publication

The actual product experiment separately uses two locked Rust 1.99.0 test-profile builds of complete source snapshots, identical external report instrumentation and unchanged original assertions. Its 17 MiB fixture ran three times per variant; eleven other report/incomplete/failure/cancellation contracts ran once per variant. All 28 exact invocations passed exactly one test. The twelve unique test names and original test bytes were independently audited.

| Actual outer write | Before median | After median |
| --- | --- | --- |
| Wall seconds | 14.847438215 | 15.031852308 |
| CPU seconds | 14.657225 | 14.808812 |
| Peak RSS KiB | 134,712 | 134,508 |

There is no observed actual publication speedup and no claimed RSS improvement. The release standalone synthetic cohort and this product test-profile cohort differ in fixture and build mode; their absolute timings are not compared or added. Preliminary/final and incomplete/failure behavior remain covered by original product assertions. The original TempDir assertions verify MachineResult, manifests and output digests, but deleted full MachineResult files are not independently archived.

Each of the six large samples retains 23 phase records. Trace `written_output` records attempted payload at function entry and does not certify successful writes on error paths. Nested parent/child phases overlap and must not be added. The two pure JSON helper contracts do not enter instrumented publication boundaries and correctly have no trace; they still pass exactly one original test. A runner incorrectly required trace for one such passing test and stopped after its PASS record. That runner and STOP remain archived; the continuation changed only the two pure-helper trace requirements and did not rerun successful stages. Private historical full-environment records are not copied to the delivery.

## Reproduction and retained evidence

The [phase tool](../../tools/quality/gh292-phase-profile/README.md) prepares fixed fixtures, records actual compiler artifacts and generated helpers, verifies complete prepared input manifests before/after and runs three samples per fixture. Independently compare both package/build flag/helper records; compiler equality alone is insufficient. All eight tested code/config/script files in the delivery are byte-identical. The README has a documentation-only update; its new hash does not replace the old measured package hash.

[The evidence index](https://github.com/musutrade/Harness-Gate/blob/d9a932b23dcfea709175285b594ca200450a9971/docs/quality/gh-292-performance-evidence.json) gives full external paths/SHA256 values, exact medians, the four-source freeze and independent audit. Historical consumer 155–263 second mixed timings are not a serialization baseline. The historical before-v2 diagnostic keeps its own identity and is not mixed into this cohort. Earlier runner selection and wrong input-root failures remain archived; corrected parity reused the original binary. No digest ignore rule was introduced.
