# Explicit runtime probes (not container lifecycle integration)

`harness-gate adapter runtime-probe --request request.json --trusted-host host.json`
is supported on Unix only in this version; non-Unix hosts reject before reading
input or dispatching a child. It executes a fixed adapter for `gate-runtime/v1` capabilities or exact-ID inspect.
It does not select a service engine, allocate/reclaim a lease, create/remove a
container, migrate lease v2, or change the `verify` service path. Removal requests
are rejected before executable dispatch even when present in the trusted file.

The operator-protected host document is separate from the candidate request:

```json
{
  "schema": "gate-runtime-host/v1",
  "executable": "/opt/operator-protected/runtime-adapter",
  "cwd": "/opt/operator-protected/probe-work",
  "timeout_ms": 5000,
  "expected": "REPLACE WITH THE COMPLETE TRUSTED REQUEST OBJECT"
}
```

`expected` must be the actual typed request object, not the placeholder string
above. Its binding fixes protocol, adapter SHA-256/version, invocation, operation
key, generation and engine namespace; its operation fixes the exact object ID.
The candidate request must match every field. This is local operator authority,
not signature verification or namespace attestation. Never derive this trusted
file from an untrusted request or let the adapter write it. Protect the executable,
its parent directories, interpreter/runtime dependencies and working directory
from untrusted modification. Existing executable hash/identity checks are reused;
they do not establish hostile same-account isolation.

Input reads and protocol output have a 65,536-byte bound. Timeout is 1–30,000ms,
with the existing two-second output-drain budget. The child receives no inherited
environment, no extra arguments, and the serialized request plus newline on
stdin. This is an environment restriction, not an OS sandbox. A trusted adapter
can still have external side effects; a read-only protocol operation alone does
not constrain a malicious executable. No PATH/DOCKER_HOST/context/default
namespace is filled in by the host.

The host rechecks the executable before and after execution, enforces existing
process completion/cancellation and output limits, and parses original stdout
bytes through the strict runtime contract. Errors are static and never replay
adapter stderr. Duplicate JSON fields remain visible to the strict parser.
A nonzero exit, timeout, cancellation or malformed response never establishes
absence. The successful CLI output is a validated fact response, not a Core
quality PASS or authorization to delete state. In particular, `unavailable` is a
valid fact response but is not `absent`; no ownership or reclamation decision is
made here. Capability negotiation and persistent recovery remain future work.

A GateProbes runtime adapter has not been published. This entry is for explicit
protocol acceptance with disposable fixtures until joint implementation and
real-engine acceptance are complete. Do not use the old Python experiment as a
production adapter. The v2 lease path is unchanged.

The working directory must be its canonical absolute path, with no symlinked
parent components or dot-segment aliases. Protect its ancestors from concurrent
replacement; this path check is not hostile filesystem isolation.
