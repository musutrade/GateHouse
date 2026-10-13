# ADR-0042: Compile trusted workflow state through generic contracts

**Status:** Proposed for GH-180; local validation recorded in OpenSpec and the retained evidence,
hosted CI/controller acceptance pending.

**Date:** 2026-09-09

**Scope:** OpenSpec `integrate-generic-quality-into-project-workflow`, tasks 2.1–2.3.

## Decision

Use one deterministic, ecosystem-opaque compiler from validated configuration
and host/pack-owned state to existing project, policy, context and optional
contracts. Bind config/source/artifact bytes and canonical subject/series IDs;
resolve kind aliases from pack data. Do not infer semantics from language names.
[The compiler reference](../quality-compilation.md) defines the versioned digest,
trust assumptions, CLI transport and failure behavior.

The compiled evaluation path recompiles, validates evidence bindings, and joins
the direct CLI's existing Rust evaluator/report builder. It creates no policy
engine and does not extend supported metrics. Retain actual CLI differential
fixtures, including unknown metadata/custom collector/series and unsupported
capabilities, with zero unexplained semantic differences.

## Consequences and measurement review

The host must supply trusted resolved state. This issue does not implement
collector orchestration, trusted baseline providers, or `verify` integration.
Malformed optional contracts are preserved for authoritative core rejection;
a compiled bundle alone is neither certification nor approval.

Extend the measured source selection from `gh179-quality-configuration/1` to
`gh180-quality-compilation/1` by adding only `config/quality/compiler.rs`.
Include it in the config production inventory and source analyzer contract test.
Both Git-verified base and head are remeasured with the same selection. This
bounded measurement review retains all previous sources and changes no analyzer,
instrumentation, exact arithmetic, coverage threshold, CRAP limit, debt/ratchet
semantics, Rust authority or single-owner CI topology. It is a review candidate,
not automatic acceptance of a baseline.
