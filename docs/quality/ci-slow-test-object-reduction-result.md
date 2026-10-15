# Policy oracle object sharing: measured result

Local test-only candidate measured 2026-10-15 against the validated first-stage optimization. Independent report cases reference their preceding evaluate case's Python inputs and expected report. Rust borrows those inputs and still executes and compares each report independently. The parsed array is moved instead of deeply cloned. No production validation or comparison is skipped.

- JSON: 137,048,761 → 77,556,593 bytes.
- Object count: 739,331 → 441,103.
- 131 references; all 826 ordered cases expand to type-sensitive identical original inputs and expectations after normalizing only their temporary work roots.
- Three interleaved Policy test runs per variant: median 56.740 → 48.929 seconds, a 13.77% reduction. Candidate range 48.921–51.941 seconds; baseline 56.739–57.148 seconds.
- Median sampled process-tree RSS peak: 1,473,290,240 → 523,784,192 bytes, a 64.45% reduction. RSS is summed across processes, sampled every 0.2 seconds, and can double-count shared pages.
- All six exact runs retain 826 comparisons, 33 frozen cases and 31 Python reference tests. Full suite: 576 passed, zero skipped, 94.701 seconds.

The preceding full suite took 92.236 seconds in a different single run. These data do not establish a full-suite wall-time improvement. Reduced memory is the strongest observed benefit. Python generation was about 26 seconds in both one-shot equivalence runs.

The frozen candidate differs from the preceding frozen snapshot only in policy_tests.rs and tests/policy_reference.py; all production source files are byte-identical. Prior production Risk evidence remains applicable to those source bytes; no new independent coverage export is claimed for this test-only stage. Independent review approved the reference approach; adjacent-case and name assertions prevent accidentally selecting another valid case.

Raw evidence: /mnt/dev-ssd/gatehouse-slow-test-optimization-20261015/object-reduction/measurement-result.json, equivalence-result.json, source-equivalence.json and adjacent logs. Existing failures and earlier benchmark artifacts are preserved. Further optimization review is pending with OpenCode; the overall task remains active. No commit or publication performed.

## Follow-up review and measured disposition

OpenCode proposed single execution for non-evaluate capture functions, initially forecasting 1.1–1.2 seconds. A dedicated diagnostic retained all calls and timed the actual capture boundaries. The second native calls for compare, aggregate, validate_policy and _select totaled 0.135 seconds; their argument deepcopy totaled 0.054 seconds. Even removing both offers approximately 0.19 seconds on this diagnostic run, not a validated end-to-end gain. Actual captured counts were 10, 267, 14 and 8 respectively; complete oracle case counts include independently generated matrix cases and cannot substitute for capture call counts.

The proposal was not adopted. evaluate had 114 captured calls and about 3.758 seconds in second native calls, but its copied-root versus original-root behavior and native return/exception semantics remain distinct. No identity-based validation cache or reduced assertion coverage was introduced. This review does not establish that all future object-reduction opportunities are exhausted. Streaming parsing and Rust domain timings were already measured; stale projected savings in the OpenCode reports must not be added to the validated results.

Diagnostic evidence: /mnt/dev-ssd/gatehouse-slow-test-optimization-20261015/capture-diagnostic/result.json. This is a single diagnostic run, not a repeated performance benchmark. No production or test source changed during this follow-up diagnostic. Overall local implementation and this requested follow-up review are complete; changes remain uncommitted.

## Authorized single-capture implementation

The user subsequently requested implementation of the discussed non-evaluate single-execution approach. It is now implemented for compare, aggregate, validate_policy and _select. Inputs are snapshotted before the one original call; successful native return objects are returned unchanged, and known native exceptions are recorded then re-raised. evaluate and review_exceptions retain their previous paths.

Independent source review passed. Three isolated checks covered native object identity and detached snapshots, exception identity/single execution, and nested-call recording/depth restoration. All 826 ordered oracle cases remained type-sensitive equivalent, including the existing 131 references.

The initial measurement attempt failed its incorrect requirement that Rust binary hashes differ despite this being a Python-only change. That failure is preserved under single-capture/. R2 deliberately used identical verified Rust binaries and explicitly selected the frozen Python script per run. Every exact run and the complete suite recorded exactly one successful policy oracle invocation with the expected script SHA. This avoids accidentally testing the old Python script through a cached embedded snapshot path.

Three interleaved runs per variant: Policy median 48.9408 → 48.7371 seconds, a measured 0.2037 seconds (0.416%) reduction. Candidate range 48.5349–48.7387 seconds; baseline range 48.9378–51.9646 seconds. The first baseline was slower; it is retained, and the large difference in that pair is not attributed to the change. RSS remains essentially unchanged at approximately 0.524 GB. This is a small measured improvement, not a material new CI speedup.

Full suite: 576 passed, zero skipped, 90.711 seconds in one run. No additional full-suite speedup is claimed from this single timing. The runtime production binary is identical to the preceding validated candidate; no new production coverage/Risk export is claimed. Final live source hashes match the measured candidate. All authorized local implementation and checks are complete; nothing has been committed or published.

Final evidence: /mnt/dev-ssd/gatehouse-slow-test-optimization-20261015/single-capture-r2/measurement-result.json. Original equivalence evidence: ../single-capture/equivalence-result.json. Prior rejected-proposal discussion above records the historical decision before the user's subsequent implementation instruction.

## PR #71 compatibility handoff repair

The first hosted CI run rejected the modified compatibility-owned `fixtures/generic-core/differential.py` through the existing pinned plugin handoff. Local targeted tests had not exercised `run_core.py --list`. The locked file is restored byte-for-byte, without changing receipts or weakening guards. Compact temporary encoding now lives in Core-owned `tools/quality/differential_transport.py`, which imports the original corpus preparation and preserves canonical persisted reports. The Rust acceptance test invokes this new entry point. Full Core test selection and physical source-cleanup checks are included in repair validation.

The repair's complete Core suite ran 561 tests and exposed one additional inventory error: the new parallel helper and compact transport needed entries in `python-boundary.json`. Both are now registered with callers and non-authoritative roles; all other tests passed. The corrected inventory and related boundary/transport checks passed in a focused 9-test rerun. The affected Rust acceptance passed all 33 cases and 12 negative categories. No pinned receipt was rewritten.
