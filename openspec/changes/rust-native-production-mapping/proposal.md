# Rust native production mapping (GH-220)

## Why
GH-219 / PR #222 supplies syntax inventory only. Production metrics require
compiler-observed expansion, cfg selection and complete LLVM ownership, including
closures and async bodies. Expression-shaped macros are not generic expressions.

## What Changes
Execute N0–N6: reproduce pinned Arc-Admin sources; add explicit macro grammars and
compiler evidence; certify complete declared production mapping with independent series,
raw-count coverage and exact CRAP; exercise positive and fail-closed negatives.

## Impact
Development measurement tools, retained evidence and tests only. Generic Core stays
language independent. Keep line/region >=80%, CRAP <=30 and debt/delta rules.
No accepted baseline reset, reference-series mutation or Arc-Admin test changes.
ADR-0040/0044/0049 and the dogfood proposal continue to govern authority.
GH-215 remains separate; GH-221 waits for accepted complete production mapping.

## Acceptance boundary
Fixture certification is scoped to its actual compiler configuration and observed
artifacts. Arc-Admin-wide certification requires real backend evidence; unresolved
mapping or missing host capabilities remain errors and unchecked tasks.

## Goals
Prove the complete production inventory and its LLVM ownership from retained
native artifacts, including generated code and independent closure/async counts.

## Non-goals
Changing Generic Core, accepting a baseline, changing thresholds, editing app
tests, or completing GH-215 or GH-221.

## Success Metrics
All pinned input hashes match; every executable owner and LLVM region is accounted
for exactly once under documented instance merging; real native negatives fail;
required local checks pass. Full backend certification has no unresolved owners.
Fixture-only evidence does not meet that last criterion.

## Risks and related decisions
High: compiler MIR and expansion formats are unstable and generated functions may
be excluded by rustc coverage instrumentation. Pin the compiler and reject gaps.
Medium: raw artifacts are large; retain compressed evidence and hash indices.
Medium: macro aliases and dependency expansion require compiler identity, not
spelling alone. Keep syntax inventory explicitly uncertified.
The production verifier invokes local LLVM only when auditing retained binaries; it does
not execute archived programs. Its manifest anchor must come from trusted capture.

References: [ADR-0040](../../../docs/adr/0040-language-agnostic-evidence-policy.md),
[ADR-0044](../../../docs/adr/0044-trusted-quality-baselines.md),
[ADR-0049](../../../docs/adr/0049-project-owned-validation-extension-boundary.md),
[previous inventory](../rust-native-macro-inventory/proposal.md).
