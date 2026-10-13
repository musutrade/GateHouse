# Adapter Protocol

## ADDED Requirements

### Requirement: Signed, capability-scoped process boundary

An adapter SHALL declare protocol/result versions, source digest and a
signature over the canonical complete request, timeout/cancellation behavior,
artifacts, network, resources, permissions, invocation identity, nonce, and
validity window before execution. Unsupported, unsigned, expired, replayed, or
tampered requests SHALL fail closed.

#### Scenario: Reject an unsigned or unsupported declaration

- **WHEN** the host receives an adapter declaration without a trusted
  signature or with an unsupported protocol version
- **THEN** the host rejects it before starting the adapter and emits a
  structured `ADAPTER_PROTOCOL_FAILURE`

#### Scenario: Request fields cannot be changed after signing

- **WHEN** an attacker changes arguments, input, environment, capabilities,
  timeout, invocation identity, or artifact root in a signed v2 request
- **THEN** signature verification fails before process start

#### Scenario: Expired or replayed request is rejected

- **WHEN** a request is outside its signed validity window or its nonce has
  already been claimed by the host policy
- **THEN** the host rejects it with `ADAPTER_PROTOCOL_FAILURE`

### Requirement: Bounded evidence and unified redaction

The host SHALL cap each captured output stream and the aggregate adapter
artifact bytes, expose a structured truncation failure, enforce an independent
reader deadline, and apply the shared redaction policy to audit, parsed-log,
report, webhook, and error outputs.

#### Scenario: Noisy or leaking adapter fails closed

- **WHEN** an adapter exceeds an output/artifact budget or emits a credential in
  a published diagnostic
- **THEN** execution fails with a truncation marker or the credential is
  replaced by `[REDACTED]`, and no raw secret is exported

### Requirement: Failure isolation

An adapter crash, timeout, malformed response, or artifact path escape SHALL
fail only its node and SHALL preserve scheduler dependency, cancellation,
redaction, lease, and evidence rules.

#### Scenario: Isolate a crashed adapter

- **WHEN** an adapter exits unexpectedly after its node starts
- **THEN** the node is marked failed, dependent nodes follow normal scheduler
  rules, and unrelated nodes continue without sharing logs or artifacts

#### Scenario: Reject an escaped artifact

- **WHEN** an adapter response references a path outside its invocation root
- **THEN** publication fails closed, the path is not exported, and the
  invocation retains a reportable protocol failure
