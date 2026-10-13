# ADR-0041: Version the quality configuration plane independently of flow

**Status:** Proposed for GH-179; implementation and local validation recorded in the PR, hosted CI/controller acceptance pending.

**Date:** 2026-09-09

**Scope:** OpenSpec `integrate-generic-quality-into-project-workflow`, tasks 1.1–1.4.

## Context

Flow v2 models execution. Generic quality needs stable component/subject identity,
measurement expectations and policy participation without embedding language
commands or giving collectors delivery authority. Existing flow-only repositories
must remain compatible. The [Engineering Policy](../engineering-policy.md) and
[ADR-0040](0040-language-agnostic-evidence-policy.md) constrain this addition.

## Decision

Define an independently versioned, closed `.harness-gate/quality.toml` v1 Rust model
and generated schema. Explicit file presence activates cross-plane config checks.
Named components map to flow components; profiles map to flow step profiles.
Collectors declare capabilities and exact measurement series. Policy bindings
reference existing `harness-policy/v1` rule documents, which remain the single
source of requiredness, limits and ratchet intent. Config validation reuses the
frozen strict parser and policy shape validator without evaluating thresholds.

Reject unresolved references, missing required producers, conflicting ownership,
incompatible series, escaping paths, inactive discovery dependencies and
baseline/ratchet contradictions. Export schemas without requiring project config;
keep existing flow export and print defaults. The
[configuration reference](../quality-configuration.md) specifies fields and limits.

## Consequences

The quality plane is language-neutral and policy authority stays outside
collectors. Existing presets and flow-only verification remain unchanged. Schema
sync and the executable quality example extend existing validation jobs, preserving
single-owner measurement and the lightweight Required Quality Aggregate.

This decision covers configuration only. Compilation, signed request orchestration,
loaded baseline trust, evidence evaluation and project verify/report integration
remain later [OpenSpec tasks](../../openspec/changes/integrate-generic-quality-into-project-workflow/tasks.md).
Configuration validation cannot certify a collector or replace the existing Rust
release gate. It does not amend CRAP, coverage, debt, ratchet, fail-closed or
measurement-series semantics.

## CI repair and measurement review

PR #188's first candidate exposed noncanonical temporary roots on macOS and
Windows. Quality loading and validation now canonicalize the repository root
before containment checks. A symlink-root regression reproduces this on Linux
and also checks that escaping child symlinks remain rejected.

The bounded source-selection policy delta is `gh179-quality-configuration/1`:
retain every GH-151 selected source and add `config/mod.rs` plus the four
production `config/quality/` files. These validators must receive actual function
risk evidence, rather than an unsupported-source exemption. Both base and head
are remeasured under the same selection; old-selection reports cannot be compared
as if compatible, and absent base files remain explicit and Git-verified.
Analyzer, instrumentation, exact arithmetic, 80% line/region coverage, CRAP <=30,
debt/ratchet rules and Rust authority remain unchanged. This is a review-only
candidate, not automatic baseline acceptance.

The analyzer contract test parses all four new sources, checks closure
instrumentation, and confirms the model has no handwritten executable symbols.
The model remains production in the coverage inventory with the same hash-pinned
unmapped-declaration treatment as existing type-only files. Changes to that file
require renewed mapping review. Only its existing `#[cfg(test)]` tests module is
excluded from production risk. Required hosted platform tests and single-owner
CI topology remain unchanged; the existing collection owner measures the added
sources without another job or collection stage.

Isolated base/head archives also retain the committed `schema/` directory because
the configuration schema synchronization test includes that compilation input.
The schema bytes come from the same commit as the measured source, never from a
different checkout or the current worktree.
