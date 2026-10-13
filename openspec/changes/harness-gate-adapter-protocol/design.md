# Design: Adapter Protocol

## Transport

One UTF-8 JSON request is written to stdin. The adapter writes one JSON result
to stdout and diagnostic logs to stderr. A cancellation signal is delivered by
closing stdin followed by a bounded process-tree termination.

## Request

Protocol v2 contains `protocol_version`, adapter name/version, invocation ID,
step ID, timeout, configuration digest, artifact root, arguments, input,
environment, explicit arrays for network/resource/environment capabilities, a
single-use nonce, and an issued/expiry window. The Ed25519 signature covers a
canonical serialization of every request field except the signature bytes
themselves. Unknown fields, invalid environment keys, expired requests, and
replayed nonces are rejected before process start.

## Response

The response contains machine-result schema version 1 fields, final status,
attempt history, parser/completeness evidence, and invocation-relative artifact
references. Harness-Gate applies one redaction policy, bounded output/artifact
budgets, size/digest manifest generation, and path containment checks before
publication. stdout and stderr readers have an independent deadline so an
escaped descendant cannot make the host wait indefinitely.

## Failure and upgrade

Signature, version, validity/replay, malformed-response, timeout, crash,
output-limit, reader-deadline, and artifact-escape failures map to
`ADAPTER_PROTOCOL_FAILURE` and fail only the adapter node. A new adapter digest
is activated through a reviewed configuration change; the previous signed
record remains available for rollback. Capability declarations and process
groups are protocol/cleanup controls, not an operating-system sandbox.
