# Proposal: Consolidate Generic Quality Core into Rust

**Status:** Tasks 1.1–6.3 are accepted through merged PRs #153–#158 with required CI complete. GH-152 implements final Python dispositions and retention policy (tasks 7.1–7.2) and prepares task 7.3 closure. [Final evidence](../../../docs/quality/gh-152/README.md) records actual validation. Full change acceptance and archival remain conditional on successful required CI for GH-152 and controller merge; this submission does not claim that future result.
**Date:** 2026-09-08
**Baseline:** `main` after TypeScript/Angular reference-adapter closure (`817190f2e26d176bb227858647c0f5a61b31ba26`).

## Why

Harness-Gate now has two real ecosystems proving the language-agnostic evidence and policy architecture: the existing Rust reference path and the bounded TypeScript/Angular reference adapter. That work also exposed a product-boundary problem.

The repository still describes `tools/quality/*.py` as quality-evidence and CI orchestration that is not linked into the released `harness-gate` binary. That remains true for many scripts, but several Python modules now implement generic product semantics rather than tooling-only behavior: evidence validation, project/subject identity, typed policy evaluation, gate aggregation, baseline/debt/ratchet decisions, cross-component relationship semantics, and project reporting.

The intended architecture is simpler: collectors may be implemented in any language, but Harness-Gate itself owns the authoritative generic semantics. Leaving those semantics split between Rust and Python creates two runtime cores, duplicate compatibility obligations, and a future risk that a user can install the Rust binary without the code that actually defines part of Harness-Gate's decision model.

This change therefore consolidates generic product semantics into the Rust Harness-Gate core while deliberately preserving Python for CI orchestration, ecosystem adapters, fixtures, evidence generation and differential-reference tooling.

## What Changes

- Define a product-boundary inventory for every production-like Python quality module and classify it as CI/dev tooling, ecosystem adapter, generic product semantics, or migration/reference tooling.
- Implement Rust equivalents for the generic semantics currently represented by `harness_evidence.py`, `project_model.py`, `policy_engine.py`, `policy_ratchet.py`, `cross_component.py`, and `project_report.py`.
- Preserve the accepted JSON schemas and machine contracts (`harness-project/v1`, `harness-evidence/v1`, policy/result/report contracts) unless a separately reviewed delta is required.
- Keep collectors language-neutral. TypeScript/Angular and future ecosystem adapters may remain external/Python processes using the versioned collector protocol.
- Run Python and Rust semantic engines over the same retained Rust and TypeScript/Angular evidence corpus in differential shadow mode before any authority transfer.
- Require exact equivalence for accepted states, blockers, typed comparisons, measurement errors, debt classifications, lineage decisions, cross-component results, report indexes and evidence links.
- Move authority only after bounded replay acceptance; mismatches block migration.
- After acceptance, retire or freeze Python copies of generic semantics so they cannot silently become an independent authoritative implementation.

## Capabilities

### New Capabilities

- `rust-authoritative-generic-quality-core`: Rust-native generic evidence, project, policy, ratchet, relationship and report semantics with differential acceptance against the established Python reference behavior.

### Modified Capabilities

Existing language-agnostic project/evidence/policy capabilities keep their accepted external schemas and behavior. This change relocates authority; it does not redefine accepted metrics or certify new ecosystems.

## Goals

1. The released Rust Harness-Gate binary contains the complete generic decision model required to evaluate normalized evidence.
2. External collectors remain implementation-language independent and do not own final delivery decisions.
3. Existing Rust and TypeScript/Angular retained evidence produces equivalent machine outcomes under Python and Rust during migration.
4. Requiredness, fail-closed behavior, capability-state handling, series compatibility, baseline/debt/ratchet semantics, exception review, contract relationships and project aggregation remain unchanged unless an explicit reviewed delta says otherwise.
5. Python remains a supported implementation language for CI helpers and ecosystem adapters without being a second authoritative core.
6. The migration does not weaken the existing Rust `Required Quality Aggregate` or change branch protection as a side effect.

## Non-goals

- Do not rewrite every Python quality script in Rust.
- Do not move Angular/TypeScript collector parsing into Rust.
- Do not replace Cargo, nextest, llvm-cov, Angular CLI, Istanbul or contract tools.
- Do not add Python/Java/Go ecosystem certification.
- Do not redefine CRAP, coverage, complexity, contract or critical-path measurement semantics.
- Do not alter required-check names or baseline policy during the consolidation itself.
- Do not delete Python reference code until differential acceptance and rollback evidence exist.

## Success Metrics

| Area | Acceptance |
| --- | --- |
| Product boundary | Every `tools/quality/*.py` production-like module is classified A/B/C/D with owner and disposition. |
| Rust core | Rust owns evidence validation, project identity, policy evaluation, ratchet/debt, cross-component semantics and project reporting used for final generic decisions. |
| Contract compatibility | Existing accepted JSON schemas and result/report shapes remain compatible or have separately reviewed deltas. |
| Rust corpus | Retained Rust replay produces zero unexplained Python/Rust semantic mismatches. |
| Frontend corpus | Retained TypeScript/Angular replay produces zero unexplained Python/Rust semantic mismatches. |
| Negative matrix | Missing/stale/tampered evidence, unsupported capabilities, series mismatch, ambiguous identity, invalid exceptions and contract failures remain fail-closed. |
| Authority | External collectors measure; the Rust core decides. Python generic semantics are non-authoritative after migration. |
| CI | Existing required Rust aggregate remains unchanged until a dedicated authority-transfer acceptance step. |

## Risk Assessment

**Risk: High if treated as a rewrite; Medium if treated as a compatibility migration.** The dangerous failure mode is semantic drift: a cleaner Rust implementation that subtly changes old outcomes. Controls are strict scope, retained evidence reuse, differential replay, exact typed comparisons, explicit migration phases and fail-closed authority transfer.

A second risk is over-consolidation. Python is appropriate for adapters, tool invocation and CI glue. The objective is not language purity; it is a single authoritative product core.

## Rollout Constraint

Migration follows an oracle model:

```text
same retained project + policy + evidence + base context
                     |
            +--------+--------+
            |                 |
   Python reference       Rust candidate
   (current oracle)       (shadow)
            |                 |
            +--------+--------+
                     |
              differential compare
                 |          |
               equal      mismatch
                 |          |
              continue   BLOCK TRANSFER
```

No required gate or release authority changes merely because Rust code exists. A separate acceptance task must prove both real ecosystems and the negative matrix before Python generic semantics can be demoted to reference/compatibility tooling.

## Related Records

- `docs/quality/architecture-closure.md`
- `docs/quality/project-model.md`
- `docs/quality/project-reporting.md`
- `docs/quality/collector-protocol.md`
- ADR-0040: language-agnostic evidence/policy architecture
- `openspec/changes/typescript-angular-reference-adapter/`
- `tools/quality/README.md`
