# ADR-0033: Signed Out-of-Process Adapter Protocol

## Status

**Implemented** (2026-08-31); request-integrity hardening accepted in
[ADR-0037](0037-adapter-request-integrity-and-evidence-budgets.md) (2026-09-01)

## Decision

Future organization-specific runners and scanners use a signed, out-of-process
adapter protocol. Harness-Gate does not load dynamic libraries into the main
process and does not expose an unstable in-process plugin API.

An adapter receives one JSON request on stdin and returns one JSON response on
stdout. Protocol v2 signs a canonical representation of the complete request:
adapter identity, invocation and step IDs, timeout, configuration digest,
artifact root, arguments, input, environment, capabilities, nonce, and
validity window. The response uses machine-result schema version 1 and may
reference only artifacts below the invocation root. Logs are streamed on
stderr and captured through the same redaction and manifest pipeline as
built-in steps.

Before dispatch, Harness-Gate verifies the adapter binary digest and complete
request signature, checks the supported protocol/result-schema matrix, checks
the nonce validity window and replay guard, and rejects undeclared
capabilities or invalid environment keys. The digest is computed from one
held file handle opened without following symlinks; immediately before spawn
the host re-checks that the handle and the canonical path still have the same
file identity (Unix: device, inode, size, mtime and ctime; Windows: size and
mtime, with the handle opened without write/delete sharing). A rename, symlink
swap or rewrite after hashing fails with `ADAPTER_PROTOCOL_FAILURE`. The
remaining window is between that final check and `execve`; a host that must
exclude it should keep adapter executables in directories writable only by the
host. A timeout, output-budget breach, or
crash fails only the adapter node; dependency propagation, cancellation,
leases, and report publication remain scheduler responsibilities. Upgrades
require a new signed digest and can be rolled back by selecting the previous
adapter record without deleting evidence.

## Compatibility Contract

Protocol and result schema versions are independent. Additive fields are
backward-compatible within a major protocol version; breaking changes require
a new major version and an explicit migration. Protocol v1 declaration-only
requests are rejected; v2 is the supported request version. The adapter must
declare the minimum and maximum Harness-Gate versions it supports. Contract
fixtures cover PASS, FAIL, CANCELLED, timeout, malformed response, artifact
escape, crash, tampered request fields, expired requests, nonce replay, and
output-budget breaches.

## Security Boundary

Adapters run with a per-invocation working directory, no inherited secret
environment, and only declared protocol-level network/resource permissions.
The host records the source digest, signature identity, request nonce and
validity window, capability declaration, exit status, output/reader limits,
and cleanup outcome. An invalid signature, replayed/expired request,
unsupported capability, malformed response, output-budget breach, or artifact
escape is a blocking `ADAPTER_PROTOCOL_FAILURE`. These checks are not an
operating-system network, filesystem, resource, or process sandbox; process
group termination is a bounded cleanup attempt only.

## Rollout

This ADR is a P2 evolution track. Built-in runner cutover does not depend on
adapter support. The host implementation is in
`tools/harness-gate/src/process/adapter.rs`, with the
`harness-gate adapter run` entry point and deterministic fixture in
`tools/quality/fixtures/adapter/`. The protocol fixtures cover complete-request
signature and digest validation, capability rejection, malformed responses,
crash, timeout, cancellation, artifact escape, replay/expiry, invalid
environment keys, and output limits on every supported platform in CI. Adapter
execution remains opt-in and is not enabled by built-in steps.
