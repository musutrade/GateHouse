# ADR-0044: Resolve immutable baselines through ecosystem-opaque providers

**Status:** Proposed for GH-182; hosted CI/controller acceptance pending.

**Date:** 2026-09-09

**Scope:** OpenSpec `integrate-generic-quality-into-project-workflow`, tasks 4.1–4.4.

## Decision

Resolve Git refs/unique merge bases from immutable objects into a fresh external
directory, and resolve retained CI bundles using independently authenticated
manifest digests and exact host-owned base state. Bind source, config, target,
run, tool, capability, measurement series and file inventory before publishing
the existing five evaluator base inputs. The [protocol](../quality-baselines.md)
defines schemas, missing-baseline behavior and supported Git representations.

Keep provider dispatch limited to configured transport kinds. Reuse generic
evidence transport validation for arbitrary capability names; strict evaluator
validation retains the supported metric registry. Baseline resolution does not
approve quality or modify the Rust evaluator's debt, ratchet or lineage decisions.
Require explicit mappings for rename/move history through the existing contract.
Separate provenance, capability and typed-value checks into small helpers so
this extension meets the existing function-risk limit without inheriting or
waiving the previous validator's complexity debt.

## Consequences

The host authenticates the retained CI producer and supplies expectations outside
the artifact. Local hashes prove integrity against those expectations, not producer
authentication on their own. Git measurements also require a trusted bundle for
the resolved commit; this stage does not launch collectors or substitute head.
Existing `verify` integration remains a later task. Git snapshots currently reject
symlinks/submodules and non-SHA-1 identities. Missing optional baselines are explicit;
required, corrupt or incompatible inputs cannot reset historical debt.

The real-Git corpus covers unknown ecosystems/custom series and capabilities,
determinism, dirty worktrees, direct evaluator parity, debt and fail-closed cases.
The measured source selection advances from `gh181-trusted-collectors/1` to
`gh182-trusted-baselines/1`, adding `config/quality/baseline.rs` and
`config/quality/baseline/git.rs` to both risk selection and the config production
coverage inventory. Both commits use this exact selection; new source absence at
base remains explicitly verified by the existing measurement machinery. Existing
selected functions, 80% coverage thresholds, CRAP <=30, historical debt and
certification requirements remain unchanged. Validation evidence is recorded in
[GH-182](../quality/gh-182/validation.md); hosted aggregation remains pending.
