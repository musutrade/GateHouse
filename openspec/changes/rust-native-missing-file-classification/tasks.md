# Tasks (GH-221)

- [x] F0 [P0/S] Define file classification, complete inventory/mapping prerequisites, configuration identity and failures.
- [x] F1 [P0/M] Reconcile all 47 pinned source hashes and trace five historical missing files through declarations, expansions, runtime owners and CTFE definitions.
- [x] F2 [P0/M] Implement generic scoped classification; preserve missing evidence as errors and genuine unexecuted coverage as measured zero.
- [x] F3 [P0/M] Compile native positives and exercise declaration/forwarding/macros/derive/task_local/cfg/zero and omission/tampering/incompatibility negatives.
- [x] F4 [P0/M] Retain all 47 real file classifications and five-file provenance with configuration and historical limitations.
- [x] F5 [P0/M] Run lifecycle, affected tests/fmt/Clippy, strict OpenSpec and complete critical_paths inventory; submit PR and final runtime handoff for controller CI.

Implementation checks do not constitute accepted proposal implementation or GH-215 approval.

Evidence:
- F0: proposal, design and the strict-validated specification in this directory.
- F1/F4: `docs/quality/gh-221/source-reconciliation.json`, `file-summary.json`,
  `backend-classification.json.gz` and `five-file-provenance.json.gz`; all 47 hashes
  reconciled, with the source archive truncation and historical LLVM gap retained.
- F2/F3: `tools/quality/rust_native_classify.py`, real `file-classification` fixture
  and `test_rust_native_classify.py`; retained captures in the GH-221 evidence bundle.
- F5: exact commands, environment, initial failures and resolved reruns are retained
  in `docs/quality/gh-221/validation.json` and its validation log archive.
