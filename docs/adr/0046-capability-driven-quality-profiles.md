# ADR-0046: Capability-driven quality profiles and retained head evidence

**Status:** Proposed for GH-184; hosted CI/controller acceptance pending.

**Date:** 2026-09-09

**Scope:** OpenSpec `integrate-generic-quality-into-project-workflow`, tasks 6.1–6.4.

## Decision

Profile metadata selects collectors, exact target/capability/series policy
bindings and assurance (`complete` by default, or explicit `partial`). Complete
profiles enforce all configured required policies. Partial profiles may omit
work but never report full-quality PASS. No ecosystem or profile name dispatch
chooses these semantics. Empty selections preserve omission without weakening
the released core's nonempty policy/evidence contracts.

The host can pin previously authenticated producer responses and artifacts for
the current binding. Generic collection validates all retained evidence before
launching missing producers, skips equivalent collection, preserves provenance
and fails closed on invalid retention without fallback. The released evaluator
keeps exclusive decision authority. Signed requests and host-owned state remain
trust inputs; a producer cannot authenticate its own retained output.

## Consequences

Required certified Rust coverage/CRAP policy participates in default complete
full/ci metadata with exact existing limits and series. TypeScript CRAP remains
unsupported. The unknown-ecosystem fixture proves both assurance levels and
arbitrary profile selections without changing core code. Complete is a stronger
validation default; configurations intentionally omitting required work must
explicitly declare partial assurance.

The [profile contract](../quality-profiles.md) defines reporting, retention trust
and reuse compatibility. Tests cover omission, selected-policy failure, reference
series parity, mixed fresh/retained producers and tamper rejection. The accepted
single-owner CI topology and lightweight Required Quality Aggregate are unchanged.
Preset generation and hosted integration remain subsequent OpenSpec tasks.
See [validation evidence](../quality/gh-184/validation.md).
