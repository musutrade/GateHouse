# Design: Adapter Request Integrity and Evidence Budgets

## Signed request

Protocol v2 serializes a fixed field-order request envelope with a domain
separator. The nested signature algorithm and key ID are included in the
unsigned envelope while the signature bytes themselves are omitted. BTree
collections and serde JSON object ordering provide deterministic encoding.
Signers use compact UTF-8 JSON with the domain
`harness-gate/adapter-request/v2`; the exact envelope order is documented in
the configuration reference so non-Rust callers can produce the same bytes.
The host verifies the executable digest, signature, validity window, capability
allowlist, environment syntax, and nonce claim before spawning the process.

## Replay and time

Every request carries a signer-generated nonce, `issued_at_ms`, and
`expires_at_ms`. The host rejects future, expired, overlong, or malformed
windows and claims the nonce atomically in a policy-scoped replay guard. The
CLI persists claims in a request-adjacent sidecar; a long-lived caller may
point the same mechanism at its control-plane storage.

## Evidence limits

Shared reader threads retain at most 16 MiB per stream by default and signal the
waiter immediately on overflow. Adapter artifacts are checked as confined
regular files and have a 64 MiB aggregate default budget. Reader completion has
an independent two-second deadline. Published invocation evidence has a
256 MiB aggregate budget and each file is checked against its 16 MiB limit
before redaction. Overflow and deadline failures are structured protocol
failures; partial byte counts are retained in the failure message for
auditability.

## Redaction

`utils::redaction` owns the credential patterns and text-size limit. Audit
violations, parsed-log fields and fallback lines, verification evidence,
webhook bodies, and audit error messages all pass through the same function
before publication or terminal output. The Python release SBOM generator
separately strips URL userinfo because it runs outside the Rust process.

## Capability wording

The implementation promises an independent process, cleared inherited
environment, protocol-level allowlists, and bounded cleanup attempts. It does
not promise OS-enforced network/filesystem/resource/process isolation until a
future platform-specific sandbox design is implemented.
