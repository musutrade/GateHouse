# Proposal: Language-Agnostic Evidence and Policy Architecture

**Status:** Closure prepared in GH-119 — tasks 0.3–10.4 accepted through merged PRs and successful required CI; task 10.5 creates the independent TypeScript/Angular proposal. Final required CI and controller acceptance remain pending. Generic stability across ecosystems and required-gate migration are not claimed. See the [closure ledger](../../../docs/quality/architecture-closure.md).
**Date:** 2026-09-08  
**Review baseline:** `ad54d8df6d21d3f6e3a0b5ee83918ae078a84d61` (`main`)  
**Depends on:** the completed `strict-json-results-and-risk-based-quality-gates` delivery series and its accepted/recorded Rust quality contracts.  

## Why

Harness-Gate has evolved from a Rust-focused quality gate into an evidence-driven delivery control plane. The current repository already has important foundations: versioned measurement contracts, production-source boundaries, fail-closed evidence validation, function-level risk evidence, critical-path traceability, and a Required Quality Aggregate. Those capabilities are valuable, but many of their current implementations and data contracts are still shaped by the first reference ecosystem: Rust.

Future projects may combine Angular/TypeScript frontends, Rust APIs, Python workers, Java services, generated clients, OpenAPI contracts, browser tests, security scanners, mutation systems, or additional languages that are not known today. If the Harness-Gate core continues to encode tool- or language-specific assumptions, each new ecosystem will require core changes, duplicate policy logic, and incompatible evidence formats.

This change therefore defines a language-agnostic architecture in which collectors produce verifiable facts, normalized evidence carries common semantics, and the Harness-Gate policy engine alone decides delivery status. Rust remains the first reference implementation; it is not removed or weakened.

## What Changes

- Introduce a versioned project/component/subject model that supports multi-component and polyglot repositories.
- Introduce a language-agnostic `harness-evidence/v1` envelope for normalized evidence while preserving raw tool artifacts and tool-specific evidence.
- Separate collectors/adapters from policy evaluation: collectors measure; Harness-Gate evaluates policy.
- Add explicit collector capability states such as `supported`, `unsupported`, `not_configured`, `not_collected`, and `measurement_error`; unsupported data is never represented as 0% or 100%.
- Generalize measurement-series identity so incompatible tool/rule/language versions cannot silently share a baseline.
- Generalize subject identity for functions, methods, files, components, endpoints, contracts, routes, and other analyzable entities without relying on short symbol names.
- Define language-independent policy inputs for coverage, complexity, CRAP/risk, mutation, security, architecture, contract compatibility, performance, and release evidence.
- Add baseline/diff/ratchet semantics that can apply across ecosystems without forcing an existing repository to clear all historical debt at adoption time.
- Define cross-component gates for contracts such as OpenAPI and generated-client compatibility.
- Make AI-agent remediation a first-class output concern: failures must identify evidence, subject, metric, base/head state, policy, and machine-readable remediation context.
- Migrate existing Rust quality functionality into the new interfaces incrementally and treat it as the reference adapter; do not rewrite working Rust evidence infrastructure solely for architectural purity.
- Preserve the current `Quality Coverage and Critical Paths` + `Required Quality Aggregate` path as the release authority during migration. The generic architecture begins as a shadow projection/evaluator and cannot weaken, waive, reinterpret, or replace an existing required result until an explicit equivalence acceptance is completed.

## Capabilities

### New Capabilities

- `language-agnostic-project-model`: project, component, target, source-boundary, subject, and cross-component relationships.
- `normalized-quality-evidence`: versioned evidence envelope, raw-artifact linkage, evidence integrity, capability status, and measurement-series compatibility.
- `collector-adapter-protocol`: collector lifecycle, requests, declared capabilities, normalized outputs, errors, and evidence retention.
- `policy-and-ratchet-engine`: language-independent metric evaluation, baseline/head comparison, debt tracking, exceptions, aggregate outcomes, and fail-closed semantics.
- `cross-component-contract-gates`: contract compatibility and evidence relationships across components such as frontend/backend or service/client.

### Modified Capabilities

- Existing Rust coverage, complexity, CRAP, critical-path and quality evidence capabilities become the first reference implementation of the generic architecture. Their accepted measurement semantics and historical series are preserved rather than silently rewritten.

## Goals

1. A repository containing Rust, TypeScript/Angular, Python and Java components can be represented without adding language branches to the core domain model.
2. Adding a new collector does not require changing policy semantics when the collector supplies an already-defined metric/capability.
3. All normalized evidence is traceable to source identity, component, commit/target, collector/tool version, rule version, measurement series and raw evidence.
4. Missing, malformed, stale, ambiguous, incompatible or unsupported evidence cannot silently become a passing result.
5. Existing Rust gates continue to work while being migrated behind generic interfaces; during shadow migration the existing Rust path remains authoritative.
6. Policies can distinguish `fail`, `measurement_error`, `unsupported`, `not_applicable`, `informational`, `warning` and `pass` without collapsing them into a single exit-code meaning.
7. Baseline/ratchet rules can prevent new quality debt while preserving explicit visibility of unchanged historical debt.
8. AI agents receive structured failure data sufficient to locate the failing subject and understand the accepted remediation paths without parsing human CI logs.

## Non-goals

- Do not implement Angular, Python, Java, Go, C#, Kotlin or any other full ecosystem adapter in this change.
- Do not replace Cargo, npm, Maven, Gradle, pytest, JaCoCo, Istanbul, mutation engines, linters, SAST tools or GitHub Actions.
- Do not create a universal AST, coverage, mutation or build engine inside Harness-Gate.
- Do not force all languages to expose the same metrics; capability differences are explicit.
- Do not redefine accepted Rust CRAP/coverage numbers merely to fit a generic schema.
- Do not accept a new organization-wide quality baseline or change branch protection as part of the architecture-only rollout.
- Do not require all historical debt to pass new policies immediately.
- Do not add a general-purpose policy programming language in v1; use a constrained, versioned policy schema.
- Do not make collectors authoritative for final pass/fail decisions.

## Success Metrics

| Area | Acceptance |
| --- | --- |
| Core independence | Core project/evidence/policy code has no required `if language == ...` branch for Rust/TypeScript/Python/Java semantics. |
| Project model | One fixture repo can declare at least four heterogeneous components and cross-component contract relationships. |
| Evidence | Generic schema validates equivalent function/method metrics from at least two synthetic ecosystems and rejects malformed/incompatible series. |
| Capability handling | Unsupported/not-collected/measurement-error states remain distinct and cannot be coerced into numeric pass values. |
| Policy | The same generic ratchet rule evaluates synthetic Rust, TypeScript, Python and Java subjects without language-specific policy code. |
| Rust compatibility | Existing accepted Rust evidence can be adapted without changing accepted raw counts, series meaning or gate outcomes; generic projection mismatches block migration rather than changing the authoritative result. |
| Fail closed | Missing base, stale evidence, duplicate subject identity, incompatible series, unknown metric and missing required artifact all fail deterministically. |
| Agent output | A failed gate includes structured subject, component, metric, base/head values, policy, evidence links and remediation classes. |
| CI rollout | New generic architecture reuses current candidate/raw evidence in shadow mode; `Required Quality Aggregate` remains authoritative until a separately reviewed equivalence acceptance authorizes replacement. |

## Impact

The expected implementation primarily affects the Harness-Gate quality/evidence architecture and configuration model. Existing Rust collectors and reports remain functional while wrappers/adapters are introduced around them. New generic schemas, policy types, component configuration and evidence validation will be added alongside existing contracts during migration.

CI impact must be measured. The architecture should not duplicate expensive collection work solely to produce generic evidence; normalizers should consume already-produced raw evidence where possible. New adapters are developed and versioned independently of release binaries when practical.

Documentation must clearly distinguish architecture adoption from ecosystem support. A generic core does not imply that Angular, Python or Java collectors are already certified.

## Risk Assessment

**Risk: Medium–High.** The largest risk is architectural overreach: attempting to generalize every current Rust detail before proven cross-language use. Controls are incremental migration, compatibility fixtures, shadow mode, explicit non-goals, and a requirement that at least one later non-Rust reference adapter validate the abstraction before declaring the architecture stable.

A second risk is semantic dilution: forcing different tools into superficially identical metrics. Controls are measurement-series identity, capability declarations, raw evidence retention, and language/tool-specific measurement contracts underneath the common envelope.

## Rollout Constraint

The previous entry condition is now satisfied: the earlier Issue series is closed and this
proposal is reviewed against `main` at
`ad54d8df6d21d3f6e3a0b5ee83918ae078a84d61`. This only authorizes architecture
planning and subsequent Issue decomposition; it does not itself authorize a wholesale
core rewrite.

During implementation, the existing Rust CI contracts are the compatibility oracle:

```text
same commit / same raw candidate evidence
             |
      +------+------+ 
      |             |
current Rust path   generic projection/evaluator
(authoritative)     (shadow)
      |             |
      +------+------+ 
             |
       equivalence check
       |            |
     equal       mismatch
       |            |
 continue shadow   block migration
```

A mismatch is a compatibility failure to investigate. The generic path must not convert
an existing `fail`, `measurement_error`, unsupported limitation, or historical debt into
a pass. Expensive Rust collection should not be duplicated merely to feed the shadow
path; adapters should project already-retained evidence whenever possible.

## Related Records

- Current risk/quality proposal: `../strict-json-results-and-risk-based-quality-gates/proposal.md`
- Current risk/quality design: `../strict-json-results-and-risk-based-quality-gates/design.md`
- Existing quality measurement and source-boundary documentation under `docs/quality/`
- ADR-0025: Phase 1 quality gates
- ADR-0039: Required risk and traceability gates
- ADR-0034: Fail-closed trust boundaries

## Closure and next change

The [acceptance ledger](../../../docs/quality/architecture-closure.md) links every
preceding task range to merged PR, required CI and validation evidence. The
[independent TypeScript/Angular proposal](../typescript-angular-reference-adapter/proposal.md)
validates the abstraction with real second-ecosystem collection; its tasks are
unimplemented. Its design explicitly records identity and capability mismatches.
Final closure requires GH-119 required CI and controller acceptance.
