# ADR-0037: Bind Adapter Requests and Bound Evidence

## Status

**Accepted / Implemented** (2026-09-01)

## Context

The first adapter protocol signed only the executable declaration. A request
file could therefore be changed after signing in fields that influence command
execution, capabilities, environment, timeout, or invocation attribution.
Process readers also used unbounded buffers, and standalone audit/log paths did
not share the verification redaction policy.

## Decision

Protocol v2 signs a deterministic envelope containing every execution-affecting
request field: adapter identity and signature metadata, protocol/result
versions, invocation and step IDs, timeout, configuration digest, artifact
root, arguments, input, environment, capabilities, nonce, and issuance/expiry
timestamps. Protocol v1 is rejected. The host validates the executable digest,
signature, time window, capability and environment policy, then atomically
claims the nonce in a policy-scoped replay guard before spawning the process.
The signed envelope uses the domain `harness-gate/adapter-request/v2` and a
fixed compact-JSON key order; the configuration reference documents the exact
ordering for non-Rust signers.

All process output is read through bounded shared readers. stdout and stderr
default to 16 MiB each, adapter artifacts default to a 64 MiB aggregate, and
reader completion has an independent deadline. Published invocation evidence is
bounded to 256 MiB, with each evidence file capped at 16 MiB before redaction.
Overflow/deadline outcomes are structured failures and cannot become a pass.
Audit reports, parsed logs,
verification evidence, webhooks, and error contexts use one redaction module;
the release SBOM generator strips URL userinfo before publishing dependency
source references.

Capability allowlists remain protocol declarations. Independent process groups
and cleanup attempts are not an OS network, filesystem, resource, or complete
descendant sandbox. A future platform-specific sandbox requires a separate
decision and tests.

## Consequences

Requests are attributable and resistant to field tampering, expiry, and
repeated CLI replay. Noisy or hostile adapters cannot grow host buffers without
bound, and exported diagnostics have a consistent credential policy. Existing
protocol-v1 request files require regeneration and signing under v2. Long-lived
orchestrators can point the durable sidecar at their own control-plane ledger.

## Rollback

Use a previous signed adapter record through a reviewed configuration change and
retain invocation evidence. Do not restore protocol v1 for untrusted request
sources.

## Evidence

Implementation: `tools/harness-gate/src/process/adapter.rs`,
`tools/harness-gate/src/process/capture.rs`,
`tools/harness-gate/src/process/reader.rs`,
`tools/harness-gate/src/utils/redaction.rs`, and the audit report/parser
modules. The release SBOM source sanitizer is covered by a Python regression
test. Focused regression tests cover tampered fields, validity/replay, invalid
environment values, output limits, and redaction in JSON/Markdown and parsed-
log fallback output.
