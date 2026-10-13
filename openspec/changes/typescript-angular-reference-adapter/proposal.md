# Proposal: TypeScript/Angular Reference Adapter

**Status:** Tasks 1.1–4.2 accepted with merged PR and successful required CI evidence. Task 4.3 bounded capability review recorded; final required CI and controller closure remain pending. No universal frontend certification or required-gate migration.
**Date:** 2026-09-08
**Depends on:** acceptance of `language-agnostic-evidence-policy-architecture`
(GH-119); no required-gate migration is implied.

## Why

Rust replay and synthetic polyglot JSON validate contracts but cannot establish
that real frontend measurements fit them. A runnable Angular application must
exercise TypeScript source identity, instrumented coverage, template boundaries,
and frontend/backend contracts through the existing collector and policy APIs.

## What Changes

- Build a small, repository-owned Angular reference fixture with pinned tools
  and real build/test/coverage output, then implement its development-only adapter.
- Feed validated `harness-evidence/v1` into the existing generic policy, ratchet
  and project-report engines alongside the Rust reference adapter.
- Define TypeScript-specific measurement series and an explicit support matrix.
- Resolve or explicitly defer the design mismatches in [design](design.md);
  require reviewed generic contract changes when existing contracts are inadequate.

## Capabilities

### New Capabilities

- `typescript-angular-reference-adapter`: real frontend evidence, source mapping,
  policy integration and bounded second-ecosystem acceptance.

### Modified Capabilities

None authorized at proposal creation. Any needed generic schema/protocol change
must first have its own reviewed spec delta and Rust regression evidence.

## Goals

Prove the abstraction with real TypeScript and Angular measurements, reproducible
raw artifacts, generic pass/fail and debt decisions, and fail-closed negative cases.
Demonstrate a real generated-client/provider contract failure blocking an otherwise
green mixed project. Keep all release authority in the current Rust required path.

## Non-goals

No Python/Java adapters; no full Angular feature or platform certification; no
universal risk series; no required-check or baseline replacement. Browser E2E,
SSR, accessibility, mutation, security and template branch coverage are outside
initial certification unless separately measured and accepted. This proposal
does not install packages, create an application or enable an adapter in GH-119.

## Success Metrics

Two retained base/head pairs (one compatible pass, one intentional regression)
from a real locked fixture reproduce native counters and generic outcomes.
Negative source-map, identity, provenance, unsupported-state and series-change
cases all block as specified. A real contract break blocks project pass.
Every claimed capability has raw evidence and a bounded acceptance record;
no unresolved mismatch is concealed by a core language branch.

## Impact and Risk Assessment

**Risk: Medium–High.** Mapping emitted code to source and overstating template
coverage are the main semantic risks. Retain source maps and original bytes,
reject ambiguity, and certify only measured capabilities. Node/tool installation
adds CI cost and dependency exposure: lock versions and isolate the advisory job;
reuse each collection for normalization instead of running coverage twice.
Adapter code owns tool parsing; existing core code owns policy and aggregation.

## Related Records

- [Architecture closure evidence](../../../docs/quality/architecture-closure.md)
- [ADR-0040](../../../docs/adr/0040-language-agnostic-evidence-policy.md)
- [Collector contract](../../../docs/quality/collector-protocol.md)
- [Project model](../../../docs/quality/project-model.md)
- [Design and mismatch register](design.md)
- [Acceptance spec](specs/typescript-angular-reference-adapter/spec.md)
- [Implementation tasks](tasks.md)
- [Final bounded acceptance review and predecessor CI](../../../docs/quality/gh-136/README.md)
