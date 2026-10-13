# Adapter Request Integrity and Evidence Budgets

## ADDED Requirements

### Requirement: Complete request integrity

The host SHALL accept only protocol v2 requests whose canonical signature
covers adapter identity, invocation and step IDs, timeout, configuration
digest, artifact root, arguments, input, environment, capabilities, nonce, and
validity window. Any changed field SHALL fail before process start.

#### Scenario: A signed execution request is tampered with

- **WHEN** any execution-affecting request field is changed after signing
- **THEN** the host emits `ADAPTER_PROTOCOL_FAILURE` and does not spawn the
  executable

### Requirement: Validity and replay protection

The host SHALL reject malformed, future, expired, or overlong validity windows
and SHALL atomically reject a nonce that has already been claimed by the host
policy.

#### Scenario: A request is replayed or expired

- **WHEN** the same nonce is presented twice or the signed expiry is outside
  the allowed clock window
- **THEN** execution fails closed with a structured protocol failure

### Requirement: Bounded and redacted evidence

Process output, adapter artifacts, request input, parsed log lines, and
published invocation evidence SHALL be bounded by host-owned limits. A limit
breach or reader deadline SHALL not be reported as a pass. Text emitted through audit, parse-logs, verification,
webhook, or error paths SHALL use the shared credential-redaction policy.

#### Scenario: A noisy or credential-bearing input crosses a boundary

- **WHEN** output exceeds its budget or a diagnostic contains a token, bearer,
  database URL, authorization header, or private key
- **THEN** the host fails with a truncation/deadline result or emits
  `[REDACTED]`, and the raw credential is absent from exported output
