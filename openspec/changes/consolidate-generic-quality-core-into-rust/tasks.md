# Implementation Tasks

All tasks belong to [proposal](proposal.md), [design](design.md), and the acceptance delta under `specs/`. This change is a compatibility migration, not a blanket Python rewrite.

## 1. Freeze the product boundary and compatibility corpus

- [x] 1.1 [P1][M] Inventory every production-like `tools/quality/*.py` module and classify A/B/C/D; acceptance: record purpose, callers, CI role, release impact, authoritative/non-authoritative status, and final disposition.
- [x] 1.2 [P1][M] Freeze shared positive/negative compatibility fixtures from retained Rust and TypeScript/Angular evidence; acceptance: exact inputs, expected outputs, schema versions and canonicalization rules are reviewable without recollecting expensive native evidence.

Validation: [GH-146 boundary and evidence](../../../docs/quality/gh-146/README.md); all 31 production modules inventoried and all 33 frozen oracle cases replayed successfully. GH-146 merged as PR #153 with required CI complete. Tasks 5–7 remain unaccepted.

## 2. Move evidence and project semantics into Rust

- [x] 2.1 [P1][L] Implement Rust `harness-evidence/v1` model/validation and typed value/capability/series integrity semantics; acceptance: shared valid fixtures pass, malformed/stale/tampered/unknown/incompatible fixtures fail with equivalent reason classes.
- [x] 2.2 [P1][L] Implement Rust project/component/subject/source-boundary/relationship model validation; acceptance: subject identity, path containment, duplicate/unknown references and relationship semantics match the Python reference corpus.

Validation: [GH-147 implementation and evidence](../../../docs/quality/gh-147/README.md); 323 Rust tests pass, including 399 evidence/project/series/capability differential cases derived from the frozen corpus and boundary mutations. The candidate library has no CLI dependency or release authority. GH-147 merged as PR #154 with required CI complete; this does not accept the full proposal.

## 3. Move policy and ratchet semantics into Rust

- [x] 3.1 [P1][L] Implement Rust policy schema validation, typed comparison, scope selection, `GateResult`, requiredness and aggregate semantics; acceptance: all policy fixtures produce zero unexplained semantic mismatches with Python.
- [x] 3.2 [P1][L] Implement Rust baseline compatibility, lineage, debt/trend ratchet and exception-review semantics; acceptance: legacy/new/improved debt, rename/move lineage, missing base, incompatible series and invalid exceptions match Python behavior.

Validation: [GH-148 implementation and evidence](../../../docs/quality/gh-148/README.md); 324 Rust tests and 289 Python tests pass. The 33 frozen policy outputs and reference boundary cases yield 391 differential comparisons with zero unexplained semantic mismatches; the existing 399 evidence/project/series/capability cases also pass. Formatting and Clippy pass. Requiredness remains policy-owned and current required CI authority is unchanged. GH-148 merged as PR #155 with required CI complete. Its temporary fail-closed contract callback is replaced by task 4.1 below; full proposal acceptance remains incomplete.

## 4. Move cross-component and project reporting semantics into Rust

- [x] 4.1 [P1][M] Implement Rust cross-component contract/relationship validation on normalized evidence; acceptance: green local gates plus breaking provider/consumer contract still blocks the project with equivalent provenance.
- [x] 4.2 [P1][L] Implement Rust project reporting and indexes; acceptance: component/local/cross-component aggregates, gate tables, indexes, evidence links and stable contract fields match canonical Python output.

Validation: [GH-149 implementation and evidence](../../../docs/quality/gh-149/README.md); 325 Rust tests and 289 Python tests pass. The 33 frozen cases and 31 reference tests yield 826 differential comparisons, including 133 complete project-report cases, with zero unexplained mismatches. A dedicated Rust regression confirms green local gates plus a breaking provider/consumer contract block both participants and the project with equivalent provenance. Formatting and Clippy pass. Required hosted CI on the final pushed SHA is pending; tasks 5–7 and full proposal acceptance remain incomplete.

## 5. Differential acceptance

- [x] 5.1 [P1][L] Add a non-authoritative Rust differential replay entry point and comparator; acceptance: it consumes explicit project/policy/evidence/base/selection/source/artifact context and reports field-level mismatches without recollecting native evidence.
- [x] 5.2 [P1][M] Run retained Rust-corpus acceptance; acceptance: zero unexplained semantic mismatches across positive and negative cases, with all current Rust required gates unchanged.
- [x] 5.3 [P1][M] Run retained TypeScript/Angular-corpus acceptance; acceptance: exact accepted counters and generic outcomes reproduce with zero unexplained mismatches, including source identity, unsupported states and contract failures.
- [x] 5.4 [P1][M] Run the consolidated negative matrix; acceptance: missing/stale/tampered evidence, ambiguous identity, unsupported required capability, malformed value, incompatible series, missing base, invalid exception and provenance/contract errors remain fail-closed.

Validation: [GH-150 replay and acceptance evidence](../../../docs/quality/gh-150/README.md); 329 Rust tests and 289 Python tests pass. The standalone comparator replays all 33 frozen cases (14 Rust, 16 Angular, 3 contract) with zero unexplained mismatches. Full outputs and 227 classified missing-file diagnostic variations are retained. Twelve explicit negative categories and the existing 399 evidence/project and 826 policy/report comparisons remain fail-closed. Formatting, Clippy, docs consistency and strict OpenSpec validation pass. No required CI definition or release authority changes; hosted CI on the final submitted SHA remains pending for the controller. Tasks 6–7 and full proposal acceptance remain outstanding.

## 6. Transfer generic semantic authority

GH-151 [hosted evidence and explicit decision](../../../docs/quality/gh-151/README.md)
accept candidate `0fc96d7644d623f31e31fe1a512cb68db0b2003b`: Rust shadow run
34243502830 and required CI run 34243502555 passed, with 33 corpus cases,
12 negative categories, 332 required Rust tests and zero unresolved mismatches.
Raw comparisons, exact hosted identities and rollback are retained. The operator
authorized the versioned production/risk measurement extensions. The separate
transfer commit and its final validation are recorded in PR #158.

- [x] 6.1 [P1][M] Integrate the Rust generic core into an opt-in shadow CI path; acceptance: existing `Required Quality Aggregate` name/dependencies/authority are unchanged and Rust/Python differential artifacts are retained.
- [x] 6.2 [P1][L] Perform explicit authority-transfer acceptance; acceptance: Rust and TypeScript/Angular corpora plus negative matrix are green, hosted CI is green, rollback is documented, and no unresolved mismatch remains.
- [x] 6.3 [P1][M] Route authoritative generic evaluation/reporting through Rust while preserving collector protocol compatibility; acceptance: external collectors still measure only, and final generic decisions no longer require Python C-class modules.

Validation of the separate Rust authority switch: 332 Rust tests and 293 Python
tests pass, including the complete CLI corpus with an empty executable search
path. Formatting, Clippy and docs consistency pass. Exact commands and logs are
retained in [GH-151 local transfer evidence](../../../docs/quality/gh-151/local-transfer/validation.json).
Project-local config check/verify are not applicable: no `ci` profile exists.
Final hosted shadow and required validation are recorded against the transfer SHA
in PR #158; candidate acceptance above precedes that switch.

## 7. Demote Python generic semantics safely

- [x] 7.1 [P1][M] Apply final dispositions to C-class Python modules; acceptance: each is removed, converted to a thin Rust wrapper, or frozen as non-authoritative reference code with no release-approval path.
- [x] 7.2 [P1][S] Document A/B/D retention policy; acceptance: CI/dev tooling and ecosystem adapters remain intentionally Python where appropriate, and migration/reference tooling has an explicit freeze/retirement policy.
- [ ] 7.3 [P1][M] Update architecture/ADR/quality documentation and run final strict OpenSpec validation; acceptance: product boundary is unambiguous, release binary responsibility is documented, full required CI passes, and no claim implies new ecosystem certification.


GH-152 closure evidence: [final disposition and validation record](../../../docs/quality/gh-152/README.md).
GH-151 merged as PR #158 at `63a7f9538b13d4f8888eff6b35663cda40270e70`
with required CI complete. All eight C modules are frozen non-authoritative
references, and A/B/D retention and retirement are explicit. Task 7.3 remains
unchecked until this PR's required hosted CI succeeds; documentation and local
strict validation alone do not accept the full proposal. The controller owns
final acceptance/merge/issue closure. Keep the named change available for strict
validation until then; this submission does not claim archival.
