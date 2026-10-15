# Slow-test optimization plan — 2026-10-15

Task `GateHouse/slow-test-optimization-plan-20261015`, epoch 1. Status: plan finalized after OpenCode review and coordinator evidence reconciliation. Scope: design only. No implementation, publication, or measured speedup is claimed. Source baseline and phase timings are in `ci-slow-test-diagnosis.md`; JSON composition and evidence hashes are in `ci-slow-test-json-composition.json`.

## Why the JSON is large

These are generated test transports containing inputs and full Python expected outputs, not simply application configuration. Frozen corpora are expanded for Rust comparison; policy tests additionally capture reference calls and separate report cases.

| Generated input | Cases | File bytes | Compact JSON bytes |
| --- | ---: | ---: | ---: |
| Policy | 826 | 142,714,517 | 137,027,929 |
| Differential | 33 | 104,209,188 | 65,223,368 |

Compact sizes were measured by serializing the existing parsed values with UTF-8 and compact separators. This measures byte reduction, not test speedup. Differential currently uses `replay.canonical`, which pretty-prints with indentation. Its approximately 39 MB formatting overhead is a transport concern; changing shared canonical output could break retained identities, so do not modify that shared function globally.

Policy's eight largest cases are four Rust Risk cases and their four report cases, totaling about 99.1 MB compact. Differential's four largest Risk cases total about 51.6 MB of its 65.2 MB compact payload. Most bytes therefore come from a few large real evidence/result sets, not the hundreds of small boundary tests.

Example `rust-1-head-risk` (compact bytes):

- Whole differential case: 12,911,550.
- Head input: 1,447,197, including 212 evidence records (1,030,478 bytes) and project/subject definitions (416,275 bytes).
- Expected output: 11,461,945. Policy result is 4,193,328 bytes, including 848 results and 353 violations.
- Project report is 7,268,550 bytes. It embeds that full policy result and contains a gate table copying the results (2,996,358 bytes including table keys).

Across all 30 differential cases with reports, embedded policy results exactly equal their top-level counterparts (18,635,632 compact bytes). All report gate value sequences exactly equal policy result sequences (13,067,057 bytes of gate values). These are distinct repeated subtrees, but their totals are not a verified encoding-size saving: references and indexes have overhead. Policy generation further records report arguments and expected reports as separate cases. A shallow deduplication of whole case fields would miss most nested repetition.

## Implementation sequence

### 1. Compact temporary test transport

Start with differential (approximately 37.4% fewer file bytes), then policy (approximately 4.0%). These are size reductions, not runtime speedup estimates. Add an explicit compact serializer for temporary Python-to-Rust inputs only. Keep retained fixture bytes, manifests, shared canonicalization, public reports, and expected values unchanged. Cover policy's temporary `cases.json` and differential's temporary input at its call sites. Do not claim compression removes traversal/evaluation work; it mostly removes whitespace scanning and memory occupied by source text.

Acceptance: parse original and compact forms with the same arbitrary-precision-enabled serde_json configuration and assert Value equality; do not substitute reserialized string comparison. Parse both forms and compare complete values, preserving array order, absent/null distinction, integer precision, and every case/oracle. Re-run the two exact acceptance tests (826 policy cases, 33 differential cases, zero unexplained mismatches). Record bytes, Python time, strict parse time, total time and peak RSS. Expected size targets are approximately 137 MB and 65 MB for the retained diagnostic inputs, with path-length differences documented.

### 2. Eliminate recursive reparsing in strict Rust JSON validation

First prototype retaining the initial serde Value parse and replacing the duplicate-key check's recursive RawValue allocation/reparse with one streaming traversal of the original JSON text. A walk over the already-created Value cannot detect duplicate keys, because serde has already collapsed them. This is a smaller compatibility change than immediately replacing the entire parser. Only consider a fused Value-building parser if evidence justifies the extra compatibility work.

Acceptance: nested duplicate keys, escaped-equivalent key spellings, arrays, scalar roots, trailing input, malformed syntax, recursion boundaries, arbitrary-precision integers, and serde arbitrary-precision reserved-key behavior. Preserve error class/message/precedence where observable. Compare old and candidate parsers on the exact same retained inputs under the same instrumented debug configuration, then run relevant parser tests and both exact acceptance tests. Keep Python oracle independent. Current 33.9/23.2-second strict parse versus 5.1/2.9-second plain parse motivates investigation; the difference is not a promised saving.

### 3. Trial lossless test-transport deduplication if still worthwhile

Prioritize differential report duplication: the two verified groups total 31,702,689 bytes, about 48.6% of compact input. Policy's 4.6 MB shallow duplicate-field statistic is not its nested deduplication potential. Use an explicit versioned test-only envelope with a table of repeated immutable values and references at known transport positions; do not interpret arbitrary user JSON objects as references. Encode equal input and expected subtrees independently from actual Rust outputs. Reconstruct exactly before comparison, ideally one case at a time to bound memory. Do not derive expected reports from actual results, drop repeated report tests, normalize additional errors, or alter public report schemas.

Acceptance: a round trip must reproduce every original case and oracle, including missing/null and ordering. Missing references, invalid indexes/cycles, and mutation aliasing must fail or be excluded by format design. Measure encoding cost, table size, reconstruction cost, parse/evaluation time, and RSS; abandon if reconstruction and complexity outweigh measured benefit. Re-expanding everything up front can restore the memory cost and does not reduce rule evaluation work. Keep this separate from the parser change for attribution and rollback. Per-case reconstruction bounds only expanded case data: total RSS still includes the reference table, input/tree storage, evaluation allocations and output retained by replay; a 13 MB serialized case is not a 13 MB RSS bound.

### 4. Profile and reduce work within rule execution

The policy case run takes about 51 seconds and differential replay/evaluation/comparison about 90 seconds. Measure json::domain call volume and outermost validation wall time; do not sum recursive inclusive durations as if disjoint. Add bounded phase timing around evidence validation, baseline validation, policy evaluation, conversions and report generation before selecting changes. Source shows replay separately requests validation and policy evaluation; this is a lead, not permission to skip either contract.

Prefer per-invocation indexes and shared validated internal representations if profiles identify repeated lookups/validation. No cross-run validation cache: evidence files, roots, project/expected context, clock, and inputs can change. Preserve failure ordering, duplicate/ambiguous evidence detection, provenance, and public entry-point validation. Optimize Python repeated domain/metadata checks only with measured evidence and independent oracle equivalence.

## Measurement and release gate

Each stage is independently reviewable and revertible. Keep CI parallel Risk disabled during optimization so CPU contention does not confound results. Use the same frozen source inputs and compiler/instrumentation settings; retain raw logs, failures, hashes, wall/CPU timings and peak RSS. Use at least three sequential observations per baseline/candidate for the two slow tests, interleaving order, and report median and range; do not rerun expensive full suites for every edit. After a candidate passes focused semantic checks and independently reviewed equivalence, run the relevant complete suite and one end-to-end serial Risk comparison. Full-suite correctness and stable Risk outputs are required before adoption. Publish no hosted-CI or release-mode speedup from these local debug measurements.

Coordinator owns integration and measurement. OpenCode supplies plan critique; implementation writer ownership must be explicit for the next phase. This planning task does not enable parallel execution or publish changes.

## Review reconciliation

OpenCode approved the phase ordering in `/mnt/dev-ssd/gatehouse-slow-tests-20261015/review-optimization-plan-opencode.json`. Adopted differential-first transport work, explicit Value equality, per-case reconstruction, and domain-validation profiling. Coordinator checked several review details against source/evidence:

- `differential.py` does call `replay.canonical(prepare(...))` for test input; retain the original source description.
- Duplicate detection must consume original text, not the post-parse Value tree.
- Plain-versus-strict parse timing is a diagnostic comparison, not a rigorous upper bound on reparsing cost.
- Shallow policy duplication does not estimate nested deduplication potential; serialized case size does not bound process RSS.

Planning is complete. No implementation writer has been assigned and no new product changes have been made by this planning task.
