# Proposal: Split Harness-Gate into GateHouse (core) and GateProbes (plugins)

**Status:** Proposed (evaluation). No code change and no Issue creation in this submission.
**Date:** 2026-10-11
**Baseline:** `main` @ `b854ed9`.

## Why

The repository is a single monorepo that mixes three different concerns:

1. **Generic product semantics** — the Rust `harness-gate` CLI: orchestration, protocol validation, execution budgets, result decision, thresholds and baseline comparison.
2. **Language/tool-specific measurement** — Rust, TypeScript/Angular and HTTP/JSON-contract collectors, plus their packaging, signing and release pipelines.
3. **This repository's own CI quality tooling** — Python quality gates that consume the collectors.

That mixing makes the product boundary unclear (ecosystem presets/packs and one database's rules are compiled into the generic core; core source references a collector fixture; three overlapping schema roots exist; plugin release pipelines live in the core repository) and blocks independent delivery of core and collectors.

This change defines a two-repository split — core **GateHouse** and collectors **GateProbes** — and, more importantly, the **dependency cut that must happen before any repository move**. Moving first would only convert single-repo coupling into cross-repo coupling.

## What Changes

- Establish an enforceable core/plugin boundary: core owns protocol validation, execution budgets, result decision, thresholds and baseline comparison; plugins own language/tool-specific measurement returning standard results. Plugins never decide gate pass/fail and never approve permission.
- Cut dependencies inside the current repository **before** moving anything: remove core→collector source coupling, decide schema source-of-truth per role, and separate fixture ownership.
- Move Rust, TypeScript and API-contract collection implementations, ecosystem templates, plugin fixtures, plugin schemas, and plugin build/sign/release machinery to GateProbes.
- Keep the existing `harness-gate` command, crate and legacy install entry compatible. Only repository names change to GateHouse / GateProbes at this stage.
- Change core self-CI to consume **pinned plugin release artifacts** instead of building the latest collector source each run.
- Isolate the Postgres specialization out of the generic config model without introducing a generic arbitrary-service plugin framework.
- Prove the first genuinely independent plugin chain with the **TypeScript-risk** plugin.

## Capabilities

### New Capabilities

- `core-plugin-boundary`: the host/plugin responsibility boundary, the dependency-cut-before-move rule, independent build/test/release guarantees, schema source-of-truth roles, and identity preservation across the repository move.

### Modified Capabilities

None. This change does not alter the accepted collector protocol, evidence/policy contracts, measurement semantics, gate names or release authority.

## Goals

1. Core can build and test without checking out plugin source.
2. A plugin upgrade does not require re-releasing or re-compiling core.
3. Core and plugins can release and roll back independently.
4. An incompatible protocol version produces an explicit error.
5. CI time and artifact disk usage do not increase because of the split.
6. Existing `harness-gate` CLI/package/install compatibility is preserved while the split proceeds.

## Non-goals

- Do not rename the CLI, crate or published package.
- Do not build a plugin marketplace, dynamic discovery, or a resident management service.
- Do not implement hot-plugging or require a host restart.
- Do not re-run a Python→Rust authority migration (Python is already a frozen reference; Rust is the decision body).
- Do not merge schemas by filename.
- Do not design a generic arbitrary-service plugin framework for the Postgres case.
- Do not split a third CI platform out of the core repository.
- Do not redefine CRAP, coverage, complexity, contract or critical-path measurement semantics.

## Success Metrics

| Area | Acceptance |
| --- | --- |
| Boundary | Every item is classified core / plugin / decided; plugins cannot decide gates or approve permission. |
| Dependency cut | Core source no longer references plugin fixtures; schemas have one source per role; generic decision fixtures stay with core. |
| Core build/test | Core builds and tests without plugin source present. |
| Plugin independence | A plugin version upgrade requires no core rebuild or re-release; next-run selection is sufficient. |
| CI | Core CI consumes pinned plugin release artifacts; no per-run build of the Rust measurer; CI time and artifact disk do not increase. |
| Compatibility | `harness-gate` CLI/crate/legacy install entry still work; incompatible protocol versions fail explicitly. |
| Provenance | Post-move signing identities (repository/workflow identity, certificate identity) are re-verified, not only download URLs. |

## Risk Assessment

**Risk: High if repositories are moved before dependencies are cut; Medium if sequenced as a dependency cut first.** The dangerous failure mode is turning one repository's internal coupling into cross-repository coupling that is harder to detect.

**Risk: Medium for release/signing identity.** A repository move changes Sigstore provenance and certificate identity. This must be re-checked explicitly.

**Risk: Low–Medium for CI cost.** A split that still builds collector source, or that duplicates plugin builds, can increase cost; the pinned-artifact rule is the control.

## Rollout Constraint

```text
current repository
  1. tidy protocol / schema / test ownership   (no algorithm change)
  2. prove one independent plugin chain        (TypeScript-risk)
  3. core CI consumes pinned plugin artifacts   -> then move GateProbes install/release
  4. remove compatibility paths last
     (Rust complex release chain and Postgres generalization handled separately)
```

No gate name, branch protection, release authority or measurement semantic changes as a side effect of this change.

## Related Records

- `CORE_PLUGIN_SPLIT.md` (evaluation v2)
- `openspec/changes/consolidate-generic-quality-core-into-rust/`
- `openspec/changes/language-agnostic-evidence-policy-architecture/`
- `openspec/changes/package-official-rust-collector-for-independent-delivery/`
- `openspec/changes/lightweight-collector-install-and-independent-upgrades/`
- `openspec/changes/typescript-risk-plugin/`
- ADR-0049: project-owned validation extension boundary
- ADR-0040: language-agnostic evidence/policy architecture
