# ADR-0045: Compose execution and generic quality in verify

**Status:** Proposed for GH-183; hosted CI/controller acceptance pending.

**Date:** 2026-09-09

**Scope:** OpenSpec `integrate-generic-quality-into-project-workflow`, tasks 5.1–5.5.

## Decision

Profiles configure trusted workflow state, adapter keys and optional baseline
requests. Verify authenticates those inputs and reconciles signed subject
selection with its resolved scope before execution. It then composes existing
required gates, immutable baseline resolution, signed collection, compilation
and the released Rust evaluator. Final success requires both execution and
generic quality success. Errors retain their phase and block success.

The unified machine report embeds trusted inputs, evidence, baseline resolution
and the complete authoritative project report. It preserves component and
cross-component gates, debt, ratchets and exceptions without interpreting an
ecosystem identifier. Human diagnostics render typed metric and policy records,
including base/head CRAP, limits, ratchet outcomes and remediation. Direct
`quality evaluate` consumes the same inputs and evaluation time; equivalence
tests compare its entire project report and decision.

The [workflow contract](../quality-verification.md) describes host preparation,
report paths and replay. A real signed collector fixture with an unregistered
ecosystem and retained baseline exercises this same path. A source regression
guard rejects closed ecosystem dispatch in generic verification and reporting.

## Consequences

Hosts supply fresh pinned state and signed requests for each invocation; verify
does not silently rewrite signed selection. Available baseline sources remain
in an external temporary directory named in the report for replay and must be
included in host retention/cleanup. Low-level `step run` remains an execution
interface. Repositories without quality configuration retain existing behavior;
profile lifecycle and cost refinements remain tasks 6 onward.

The risk source selection advances from `gh182-trusted-baselines/1` to
`gh183-quality-verification/1`, adding `verify/quality.rs` to the shared base/head
inventory and the blocking production coverage boundary. This retains all
existing selected functions, thresholds, historical debt and certification
rules. Local validation is recorded in [GH-183](../quality/gh-183/validation.md).
Hosted Required Quality Aggregate remains pending.
