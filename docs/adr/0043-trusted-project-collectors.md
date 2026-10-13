# ADR-0043: Orchestrate configured collectors through the signed adapter host

**Status:** Proposed for GH-181; hosted CI/controller acceptance pending.

**Date:** 2026-09-09

**Scope:** OpenSpec `integrate-generic-quality-into-project-workflow`, tasks 3.1–3.4.

## Decision

Resolve selected collector bindings and canonical capability/series metadata from
validated configuration and trusted pack state. Use the existing signed adapter
v2 boundary for launch, executable integrity, replay protection and limits. Bind
the project request to configuration, roots, source selection and package identity;
validate the entire response and normalized evidence before publishing a collection.
[The protocol reference](../quality-collectors.md) defines the exact contracts.

Ecosystem metadata remains opaque. Unknown ecosystem/custom binding fixtures use
the same generic orchestration, backed by a guard against closed language dispatch.
Collectors measure only. Capability unavailability is preserved for Rust policy
evaluation and never converted into values or uncertified CRAP. Retain `adapter
run` for debugging; `quality collect` exposes this stage independently of the
later `verify` integration.

## Consequences and measurement review

A trusted caller must supply signed requests and host-owned state. All selected
producer execution/validation failures block collection; policy still owns
whether a valid unavailable capability blocks delivery. A fresh artifact root
prevents stale evidence reuse. No additional CI collector job is introduced.

Extend the measured source selection from `gh180-quality-compilation/1` to
`gh181-trusted-collectors/1` by adding `config/quality/collectors.rs` and the
declaration-only `process/mod.rs` whose adapter visibility changes. Add the new
collector file to the config production inventory and analyzer contract test;
refresh the existing process module's declaration-only source hash. Explicitly
recognize its test module in the CI source review. Base/head measurement uses the
same expanded selection; this changes no thresholds, analyzer, certification,
ratchet semantics or Rust decision authority. This is a bounded measurement review
candidate, not automatic baseline acceptance.
