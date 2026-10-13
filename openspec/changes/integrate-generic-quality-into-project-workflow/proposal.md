# Proposal: Integrate Generic Quality into Project Workflow

## Summary

Make the released Rust generic quality core a normal project workflow capability instead of a low-level migration/API surface. A project initialized with a supported preset should be able to declare quality intent once and use `harness-gate verify` to select work, execute traditional gates, orchestrate trusted collectors, validate normalized evidence, apply policy/baseline/ratchet semantics in Rust, and emit one authoritative project decision/report.

This change inherits `docs/engineering-policy.md` and the completed CI execution-topology contract. **CRAP, coverage, debt/ratchet, fail-closed, measurement-series, and Rust authority semantics are unchanged.** This change integrates those semantics into the product workflow; it does not redefine them.

## Why

The architecture is already proven:

- external collectors can measure language/tool-specific facts without owning delivery decisions;
- normalized `harness-evidence/v1` and trusted project/policy context can be evaluated by the released Rust core;
- Rust and Angular reference ecosystems demonstrated language-agnostic evidence/policy boundaries;
- `harness-gate quality evaluate` produces the authoritative generic project decision;
- the CI topology now has explicit evidence ownership, artifact trust, cost governance, and a lightweight Required Quality Aggregate.

But the normal product UX has not caught up. Today `quality evaluate` requires callers to manually provide project, policy, evidence, expected context, source/artifact roots, output, and optional baseline inputs. Presets still primarily describe command steps. The adapter host exposes a low-level `adapter run` protocol rather than a project-level collector lifecycle. Users should not need to understand migration-era JSON plumbing to receive the quality guarantees Harness-Gate already implements.

## Goals

- Add a versioned project quality configuration (`.harness-gate/quality.toml`) that describes project/component/subject relationships, collector capability bindings, policy intent, baseline strategy, and profile participation without transferring decision authority to collectors.
- Keep `.harness-gate/flow.toml` focused on execution mechanics and `.harness-gate/quality.toml` focused on quality/control-plane semantics.
- Integrate generic quality orchestration into `harness-gate verify` while retaining `quality evaluate` as an explicit low-level/advanced interface.
- Automatically construct trusted project/policy/expected/source/artifact context from validated repository configuration and selected workflow state.
- Orchestrate signed/out-of-process collectors through the existing adapter boundary, validate capability/evidence contracts, and feed only normalized trusted evidence to the Rust core.
- Add baseline providers so normal users do not manually supply the five low-level base arguments; begin with deterministic Git/base-ref and retained-artifact strategies with explicit identity/compatibility checks.
- Make certified CRAP/risk capabilities normal required `full`/`ci` quality for applicable presets while preserving the Engineering Policy rule that expensive full measurement need not run in `hook`.
- Upgrade supported presets so generated projects receive coherent flow + quality configuration and can reach one authoritative project decision through `verify`.
- Produce unified human and machine reporting for traditional gates, component/local quality, cross-component contracts, baseline/ratchet/debt, capability states, and final project status.
- Reuse the completed CI topology: one owner per provenance-sensitive measurement series, immutable validated evidence reuse, no unnecessary duplicate collection, and explicit cost/profile placement.

## Non-goals

- Do not change CRAP `<= 30`, accepted Rust coverage/risk thresholds, debt/no-regression semantics, or existing measurement-series identity.
- Do not invent CRAP for TypeScript/Angular or any ecosystem whose CRAP capability is not certified; unsupported remains explicit.
- Do not move requiredness, thresholds, ratchets, exceptions, aggregate decisions, or release authority into collectors/adapters.
- Do not replace the Rust generic quality core with flow-step exit codes or adapter self-approval.
- Do not remove `quality evaluate`, `adapter run`, or compatibility tooling in this change; lifecycle/deprecation can be proposed separately.
- Do not add Java, Python, or a third ecosystem reference adapter.
- Do not introduce risk-driven conditional removal of currently required hosted CI assurance.
- Do not turn `flow.toml` into a combined schema v3 containing all quality semantics.

## Product model

The intended user path becomes:

```text
harness-gate init --preset <preset>
        |
        +-- .harness-gate/flow.toml       execution plane
        +-- .harness-gate/quality.toml    quality/control plane
        |
harness-gate verify [profile/scope]
        |
        +-- validate configuration + trusted project identity
        +-- select affected components/subjects
        +-- secrets/audit/traditional configured steps
        +-- orchestrate profile-applicable collectors
        +-- validate/normalize evidence and capability states
        +-- resolve trusted baseline/context
        +-- released Rust generic quality evaluation
        +-- unified project report
        |
        +-- authoritative PASS/FAIL
```

The low-level interface remains available for debugging/integration:

```text
harness-gate quality evaluate --project ... --policy ... --evidence ...
```

but normal preset users should not need to construct those files manually.

## Configuration boundary

`flow.toml` answers **how work runs**: commands, services, timeouts, dependencies, scope mechanics, execution profiles.

`quality.toml` answers **what exists and what good means**: components/subjects/relationships, certified collector capabilities, required policy bindings, baseline strategy, profile participation, and reporting intent.

The exact schema is part of this change and must remain language-neutral. Collector entries identify measurement/capability producers but cannot declare themselves authoritative or make a required result optional.

## Profile behavior

- `hook`: fast deterministic developer feedback. It may omit expensive full coverage/CRAP collection, but must never fabricate a favorable quality result for measurements not collected in this profile.
- `full`: normal complete local verification. Certified required quality measurements, including Rust CRAP where applicable, participate.
- `ci`: authoritative hosted verification. Required evidence, baseline/ratchet semantics, cross-component contracts, and final Rust project decision participate fail closed.

Profile omission is explicit configuration/policy behavior, not `skip-as-pass`.

## Acceptance

The change is complete only when:

- supported presets generate valid `quality.toml` alongside existing configuration;
- `harness-gate config check` validates flow/quality cross-references and rejects authority/capability/profile contradictions;
- `harness-gate verify` can execute at least the certified Rust and Angular/reference project shapes through the generic orchestration path without callers manually constructing low-level evaluation JSON;
- Rust projects with certified CRAP series receive the existing required CRAP/debt/ratchet semantics in `full`/`ci` by default where the preset promises that capability;
- Angular/TypeScript CRAP remains explicitly unsupported unless separately certified, with no invented value;
- missing/invalid collector evidence, expected capability gaps, incompatible baselines, stale/mixed identity, or artifact mismatch fail closed when required;
- baseline providers produce the same trusted low-level context required by `quality evaluate` and cannot silently reset debt/ratchet lineage;
- one unified machine report contains traditional workflow outcome plus generic project/component/cross-component quality state and evidence links without giving flow-step exit codes authority over generic policy;
- CI integration reuses existing measurement ownership/artifact trust and does not add redundant full collection merely because `verify` now orchestrates quality;
- documentation and CLI help make `init -> verify -> one project decision` the primary product story while preserving advanced interfaces.

## Follow-up

After this change is accepted, dogfood the workflow in a real Angular + Rust + PostgreSQL application and evaluate whether a separately governed risk-driven hosted CI policy is warranted. Additional ecosystem adapters remain independent changes.