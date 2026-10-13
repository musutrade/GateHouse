# Tasks (GH-220)

Parent: [spec](specs/rust-native-production-mapping/spec.md). Each numbered
subtask is a bounded implementation/review chunk under four hours; parent IDs
remain open until their full acceptance criteria have evidence.

- [x] N0 [P0/S] Create specification and close M4.2 only with PR #222 merge/check evidence. Strict OpenSpec validation passed; see [merge evidence](../../../docs/quality/gh-220/pr-222.json).
- [x] N1 [P0/M] Fetch pinned Arc-Admin inside this workspace; verify all 47 source hashes and reproduce committed syntax failures using the rebuilt baseline analyzer. See [baseline replay](../../../docs/quality/gh-220/source-replay-baseline.json).
- [x] N2 [P0/M] Prove reproducible syntax and expansion provenance for every listed macro, derives and cfg.
  - [x] N2.1 [P0/S] Add bounded metrics/anyhow/try_join grammars and record unresolved macro/attribute syntax; final syntax replay retains three failing files.
  - [x] N2.2 [P0/M] Retain real expanded hygiene contexts and cfg in a single-file experiment; reject missing context edges.
  - [x] N2.3 [P0/M] Resolve compiler definition IDs and invocation spans for tracing/select, task_local, macro_rules, derives and generated dependency code. Syntax allowlists do not prove resolution.
- [x] N3 [P0/M] Certify complete production function/closure/async/generated LLVM ownership and test exclusion.
  - [x] N3.1 [P0/M] Join real fixture MIR and LLVM with independent closure/async counters and generic-instance deduplication; preserve raw native artifacts.
  - [x] N3.2 [P0/M] Compile and sample existing backend contract tests; audit production-library MIR and retain unobserved/ambiguous owners.
  - [x] N3.3 [P0/M] Observe derived/generated functions and constructors with independent typed MIR block counters, including explicit coverage-off derived Eq. Reject missing constructor MIR.
  - [x] N3.4 [P0/M] Join all backend targets/dependencies through compiler owner IDs and verifiable production/test exclusion.
- [x] N4 [P0/M] Integrate complete production metrics under an independent versioned series.
  - [x] N4.1 [P0/S] Calculate bounded raw line/region/function coverage and rational CRAP; test incompatible tool/rule/cfg selection; preserve failing thresholds and old baselines.
  - [x] N4.2 [P0/M] Integrate complete production evidence with immutable historical debt/delta rules. Independent production series feeds the released Rust evaluator; real fixture history proves unchanged debt and CRAP 7 → 56 regression rejection. No backend baseline is accepted.
- [x] N5 [P0/M] Validate full production mapping positives and negatives.
  - [x] N5.1 [P0/M] Compile native fixture positives and cfg/derive cases; reject omissions, duplicate/ambiguous owners, tampered exports and incompatible history.
  - [x] N5.2 [P0/M] Supply complete real backend evidence for production/generated/test-exclusion acceptance; fixture coverage cannot substitute.
- [x] N6 [P0/M] Complete local checks and prepare scoped delivery for controller review.
  - [x] N6.1 [P0/M] Run lifecycle, analyzer, OpenSpec and complete critical_paths checks; 392 nextest and 399 Python tests passed (including nine real native compiler/policy regressions), seven analyzer tests, driver/analyzer/harness fmt/Clippy, 12 lifecycle acceptance cases, 56 strict OpenSpec items and complete critical_paths inventory passed. Exact results and preserved initial failures are in [production-validation.json](../../../docs/quality/gh-220/production-validation.json); project-local config/verify are not applicable.
  - [x] N6.2 [P0/S] Prepare indexed PR #223 with complete declared-configuration production evidence and truthful threshold failures. Runtime handoff follows the final push; controller review, CI and merge acceptance remain pending.

Completed implementation tasks N2–N5 are supported by the real pinned backend
[production evidence](../../../docs/quality/gh-220/production.md),
[expansion audit](../../../docs/quality/gh-220/production-expansions.json) and
[artifact index](../../../docs/quality/gh-220/production-artifacts.json).
The earlier prototype checkpoint remains historical evidence. Final acceptance
requires controller review and CI; GH-215 and GH-221 remain separate.
