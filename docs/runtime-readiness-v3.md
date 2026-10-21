# Staged v3 image readiness

This source candidate adds `gate-runtime/v3`, protocol crate 0.2.0, and the
GateProbes 0.2.0 `gateprobes-docker-readiness` executable. It is not a published
release. Existing v2 requests, hosts and released runtime0.1.1 artifacts retain
their contract. Capabilities never establishes readiness.

A Docker service may opt into read-only availability with `readiness_host`, an
absolute path to a protected `gate-runtime-host/installed-v2` document. Only
`doctor`/service availability uses this selection. Environment and external-env
routes retain recipe validation and bypass the runtime. Podman cannot select
this host. Failure does not fall back to a Docker CLI. Services without this
field retain the legacy CLI. Startup, health checks, leases, recovery and cleanup
remain on their existing paths; this is not default lifecycle migration or #62
completion.

The host has exactly `schema`, `installation`, `executable`, `config`, `cwd` and
`timeout_ms` (1..30000). `installation` pins the path, SHA256 and schema of an
existing protected `gate-runtime-installation/v1` descriptor. `executable` pins
path, SHA256 and version `0.2.0`, exactly matching that descriptor. `config` pins
path, SHA256 and schema `gate-runtime-engine-config/v3`. Config fields are the
v2 engine pins with the v3 schema: engine=docker, transport=unix-http, socket_path,
socket_uid, principal_uid, rootless, api_version, server_version, daemon_id, scope.
The host accepts no expected path, request, nonce, generation or operation key.
Installation provenance remains a local operator assertion, not authentication.

Core reads and holds the protected files, hashes the exact config bytes and
mints 16 bytes using Linux getrandom per call. The invocation is 32 lowercase
hexadecimal digits. Generation is SHA256 over length-prefixed UTF-8 fields:
`gate-runtime/v3/generation/1`, adapter SHA256, adapter version, config SHA256,
exact selected image. The config hash commits every engine field and its byte
spelling. Operation key uses the same encoding over
`gate-runtime/v3/operation/1`, invocation, generation, config SHA256, image.
Lengths are unsigned 64-bit big endian. No entropy failure fallback exists.
Expected bytes are held in memory and sealed fd4; sealed fd3 holds config;
stdin carries the same expected request. They are never a reusable expected
file. Host files and executable identity are revalidated around bounded execution.

The sole request operation is `{"readiness":{"image":"exact reference"}}`.
The response exactly echoes binding and operation. A readiness outcome contains
`image_present` and, only when true, a `sha256:<64 lowercase hex>` image_id.
Explicit null is not an omitted image_id. Matching before/after API, server
version and daemon ID observations are mandatory. Unavailable has null
observations. Old invocation responses, duplicate or unknown fields, trailing
objects, mutation and capabilities outcomes are refused.

The adapter performs GET /version, versioned /info, the versioned image inspect
route, then repeats the two identity observations. It uses one bounded deadline,
no pull, CLI, proxy or ambient image/context fallback. The image is a single
percent-encoded path segment (including slash, colon and @). A well-framed 404
with a valid JSON message at the exact image route means image_present=false;
other status, malformed document, timeout or changed identity means unavailable.
The image API follows Docker's [versioned Engine API](https://docs.docker.com/reference/api/engine/).
A digest reference must match a complete RepoDigests entry exactly. Tags remain
mutable; the returned local image/config ID is not a manifest provenance proof.
Image references are bounded to 512 ASCII bytes; whitespace, controls, query,
fragment, leading +/- and empty/dot/dot-dot path components are refused.

Run joint acceptance using explicitly built local artifacts:

```
python3 tools/experiments/runtime-readiness/accept.py \
  --core /absolute/harness-gate \
  --adapter /absolute/gateprobes-docker-readiness
```

It creates a protected temporary project and fake Unix socket under HOME/.cache,
exercises real doctor-to-adapter execution, and installs rejecting fake Docker
and Podman commands to detect CLI fallback. It covers present, missing,
unavailable, daemon identity change, timeout and injected fixed expected input.
Protocol, input, socket and host unit tests cover additional rejection cases.
No real daemon, rootless/restart attestation, hostile same-user isolation,
race-free ancestor traversal or OS containment is claimed. Filesystem operations
have no hard deadline. Windows Core0.8 cancellation remains separately unresolved.
The existing importer/package manifests still describe their immutable v2
releases; v3 distribution/import and default migration require separate work.
