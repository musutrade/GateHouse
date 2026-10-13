# Proposal: Adapter Request Integrity and Evidence Budgets

**Status:** Implemented
**Date:** 2026-09-01

The original adapter protocol authenticated the executable declaration but left
request orchestration fields outside the signature. Process capture and
standalone audit paths also had inconsistent resource and redaction boundaries.
This change makes the adapter request itself an authenticated, time-bounded
capability and applies bounded, redacted evidence publication consistently.

## Goals

- Reject protocol v1 declaration-only requests and require a canonical v2
  signature over every request field that influences execution.
- Bind nonce, issuance/expiry, invocation, step, configuration digest, artifact
  root, arguments, input, environment, capabilities, and timeout to that
  signature.
- Reject malformed environment keys before process creation and prevent
  requests from overriding host-owned metadata variables.
- Bound stdout/stderr, adapter artifacts, request size, and log-line size;
  expose truncation and reader-deadline failures without treating partial data
  as a pass.
- Reuse one redaction policy for audit JSON/Markdown, parsed logs, verification
  evidence, webhooks, and error contexts.
- State clearly that protocol capability checks and process-group cleanup are
  not an operating-system sandbox.

## Non-goals

- This change does not claim Linux, macOS, or Windows OS-level network,
  filesystem, resource, or process sandboxing.
- It does not move DevRail policy, required-check ownership, or release
  decisions into Harness-Gate.
- A long-lived orchestrator may provide its own durable replay ledger; the CLI
  uses an atomic request-adjacent sidecar in addition to its in-memory guard.

## Rollback

Revert the protocol-v2 implementation and select the previous signed adapter
record through a reviewed configuration change. Do not re-enable protocol v1
for untrusted request files; rollback must retain invocation evidence.
