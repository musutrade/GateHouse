# Docker lifecycle contract (implementation specification)

Status: selected design, not an implemented or released capability.

## Authority and installation

Use a new `gate-runtime/lifecycle-v1` protocol and `docker-lifecycle` product.
Neither the old runtime protocols nor readiness grants mutation authority.
Select it only through an explicit protected `lifecycle_host`; verify the
installation descriptor, executable and exact configuration snapshot. Never
resolve authority from PATH, environment, readiness_host or an old lease.

Core owns project/resource/lease identities, a fresh 128-bit invocation nonce,
operation key and generation. Each request binds those values, installation and
configuration digests, exact Docker endpoint and observed daemon identity.
Expected authority travels on the protected host channel, separately from the
adapter response. Every operation must match it and before/after observations.
A changed daemon, timeout or missing response is uncertainty, not absence.

## Creation is deliberately two operations

Create and start are separate protocol operations. Docker create is not atomic
with start, and a lost response cannot reliably carry a partial object ID.
Core durably writes a pending creation lease and complete request digest before
create. Create supplies the exact name, resolved image ID, ownership labels,
environment and port specification. No pull, auto-remove or privileged mode.
A mutable image tag is resolved and its image ID is bound before creation.

After create returns a full immutable object ID, Core persists that ID before
issuing start. It first performs an exact-ID status check matching the expected
name and all ownership labels. A start failure retains the known ID and lease.
A lost create response retains an unresolved pending lease; no automatic retry,
name lookup, list/discovery or deletion is allowed. Report operator reconciliation
required. This intentionally prefers a retained orphan over destroying a resource
whose identity was never durably established.

## Status, stop and remove

Status addresses a full object ID and returns name, ownership labels, explicit
container state and port bindings. Unknown or malformed states are unavailable.
Core checks identities and permits only the transition it requested. The adapter
independently inspects and compares exact ID and labels before every mutation.

Stop carries an explicit bounded grace period. Remove is non-forced only and
requires a fresh stopped/created/exited state; paused, running, restarting and
unknown states are rejected. There is no implicit stop in remove, volume removal,
force escalation or automatic retry. Core retains the lease after any uncertain
outcome. Release requires a new exact-ID status request establishing absence on
the same bound daemon, not just a successful remove response. Only the exact
container-inspection route's authenticated 404 establishes this absence.

Port discovery comes from status. Recipe health commands use the bounded exec
operation specified below under the same exact-ID/label authority. Until that
operation is implemented, lifecycle selection for those recipes must fail closed.
It must never silently invoke the legacy CLI. Full managed-startup acceptance
requires health-exec implementation as well as container creation and startup.

## Lease and compatibility

Use a separately identified lifecycle lease schema with explicit authority,
installation/config digests, daemon identity, pending operation and optional
object ID. Persist transitions before side effects. Existing CLI/v2 leases keep
their existing reader and cleanup path; no automatic conversion. Cancellation,
recovery selection, marker retention and cleanup reporting stay in Core.

## Acceptance sequence

First test fake backends and sockets: exact request binding, successful transitions,
lost create response, create-success/start-failure, wrong labels/ID/daemon, running
remove refusal, cancellation, deadlines, malformed response, absence routing,
crash windows around lease persistence, and no CLI fallback. Old leases must not
enter the new mutation path.

Then validate Docker with isolated real resources, including authorized rootless
and restart scenarios. Restart tests use an isolated daemon rather than a shared
business daemon. Only after Docker extraction and validation proceed to Podman.
No real-engine result is claimed by this specification.

## Lease state machine and failure policy (revision 2)

`gatehouse-lifecycle-lease/v1` is separate from every historical lease schema.
The record has exactly: schema, revision (u64), project_id, resource_id,
lease_invocation (32 lowercase hex), authority (`container-lifecycle/mutate`),
protocol, installation_sha256, executable_sha256, config_sha256, engine_binding,
name, labels, image_id, container_port, state, object_id (explicit null or full
64 lowercase hex), pending_request_sha256 (explicit null or digest),
created_at_ms and updated_at_ms. Labels are a sorted unique key/value array and
include project, resource, lease invocation and lifecycle authority markers.
Timestamps never grant authority or expiry-based reclamation.

The exclusive per-resource lock is held while transitioning. Save by exclusive
0600 temporary creation in the protected 0700 parent, fsync file, atomic rename,
fsync parent before sending any mutation. A write/fsync failure prevents sending.
The reader rejects unknown/duplicate fields, symlinks, unsafe owner/mode, invalid
state/ID combinations and unsupported schema; there is no legacy deserialization
fallback. Existing records in any non-absent state block acquiring this resource.

Transitions:

| Durable state | Next allowed event | Durable successor |
| --- | --- | --- |
| pending-create (no ID) | validated create response | known-created (full ID) |
| pending-create (no ID) | any failure or crash recovery | unresolved-no-id |
| known-created | persist start intent | starting |
| starting | fresh verified running status | running |
| any known-ID non-absent state | failure, cancellation, crash recovery | retained |
| running | persist health intent | checking-health |
| checking-health | verified completed health exec and fresh running status | running |
| retained/known-created/running | explicit cleanup, fresh identity/state proof | stopping or removing |
| stopping | fresh terminal status | stopped |
| stopped | persist nonforced remove intent | removing |
| removing | successful removal claim | awaiting-absence |
| awaiting-absence | fresh exact-ID absence | verified-absent |

Neither pending-create nor unresolved-no-id can issue another create/start or
expire. A crash after a create response but before its ID is durably committed
is unresolved-no-id. A crash after the ID commit retains that exact ID but does
not replay start. All in-flight known-ID states recover as retained, never replay.
No synchronous startup failure automatically cleans up: create/start/status/health
failure, transport error, cancellation and deadline all retain the record. A later
explicit Core cleanup action can inspect a known ID and use stop -> fresh terminal
status -> remove -> fresh absence. Any failure retains it again. This policy also
applies to startup's existing error handlers; they cannot invoke legacy teardown.
Normal successful-service teardown uses that same verified cleanup sequence.

Operator reconciliation initially means an actionable report of the unresolved
resource, request digest and saved evidence. It does not provide a bypass command
or automatically unlock a resource. Manual adoption/clearing is a future explicit
contract; absence of that feature does not authorize name-based deletion.

## Protected host and wire format (revision 2)

Host schema is `gate-runtime/lifecycle-host-v1`, with exactly schema,
installation_path, installation_sha256, config_path, config_sha256. Paths must be
absolute protected regular files (owner current UID, 0400 or 0600, no symlinks,
protected ancestor chain); executable is verified against the installation's
full SHA256 and opened identity. Installation schema is
`gate-runtime/lifecycle-installation-v1` with schema, product (`docker-lifecycle`),
version, protocol, executable_path, executable_sha256 and config_schema.
Config schema is `gate-runtime/lifecycle-config-v1` with schema and engine_binding.
Engine binding has exactly engine (`docker`), transport (`unix-http`), socket_path,
socket_uid, principal_uid, rootless, api_version, server_version, daemon_id, scope.
Rootless is an expectation requiring real validation, not an attestation.

One process handles one request. stdin carries request JSON; inherited fd4 carries
Core's immutable expected request bytes. fd4 is a read-only protected unlinked file,
never a pathname supplied by the adapter. Request and expected must match exactly
as validated structures; neither can be loaded from environment. The response on
stdout is a single JSON document, at most 65536 bytes; stderr is empty. Rejections
use a static error classification without secret/argv/env echo. Duplicate fields,
unknown fields, trailing data and invalid UTF-8 are rejected, including nested
objects. Limit request and protected input to 65536 bytes each before parsing.

Request fields: protocol (`gate-runtime/lifecycle-v1`), binding, operation.
Binding fields: installation_sha256, executable_sha256, config_sha256,
engine_binding, project_id, resource_id, lease_invocation, invocation (fresh
128 random bits per call), generation (lease revision), operation_key.
operation_key is SHA256 of domain `gate-runtime/lifecycle-v1/request\0` followed
by canonical request bytes excluding operation_key (sorted object keys, compact
UTF-8 JSON; no floats). Core writes this request digest in the lease before effects.
Expected is minted anew per call; never accepted from a public fixed host file.

Operation is exactly one tagged object (`kind` plus listed fields):
resolve-image {image}; create {name,image_id,labels,environment,container_port};
status/start/remove {target}; stop {target,grace_seconds};
health-exec {target,argv,timeout_ms}. Target has object_id,name,labels.
Environment and labels use unique sorted arrays, not duplicate-overwriting maps.
Response fields: protocol,binding,operation,outcome,observations. Observations are
required before/after engine identity facts for any successful fact/effect; a
missing observation can only produce unknown, never authorize a next transition.
Outcome is a tagged kind: image {image_id,repo_digests}; created {object};
status {object}; started/stopped {object}; removed (claim only); absent;
health {exec_id,exit_code,object}; rejected-before-effect; unknown.
Object has object_id,name,labels,state,ports. Ports is an array of
{container_port,protocol,host_ip,host_port}. Output contains no command output.
Mismatched outcome/operation is rejected. `unknown` cannot release a lease or
prove an effect did not occur. Nonzero adapter exit cannot carry trusted success.

`lifecycle_host` is only valid for managed Docker services. Reject external_env
combinations and unmanaged modes even if the environment variable is unset.
readiness_host stays read-only and cannot provide lifecycle authority. image_env
and timeout_env can select validated user values before the request snapshot;
they never select hosts, endpoints, installation, lease identity or generation.
Old host/lease schemas are always rejected by this path.

## Docker route and health rules (revision 2)

Use the configured API version and percent-encode each exact path component.
Create expects 201 with full ID; start expects 204, or 304 only with a fresh
verified running object; stop expects 204/304 followed by a fresh terminal object;
remove expects 204 followed by an independent status request. Any other status,
redirect, malformed/oversized body, route error or transport failure is unknown.

States are created,running,paused,restarting,removing,exited,dead. Only created and
exited permit remove. Dead/removing/paused/restarting reject mutation in this first
slice. Stop on created/exited is a verified no-op, not a remove; running stop
requires grace_seconds in 1..=30 and remaining operation time greater than grace.
A status 404 means absent only for GET /containers/{full-id}/json, with a bounded
Docker JSON error containing exactly message `No such container: {full-id}` and
matching before/after daemon observations. A 404 from any other route is unknown.
The daemon endpoint is explicitly trusted; message shape alone authenticates no
server. Ports accept exactly one TCP binding for the requested port with host IP
127.0.0.1 and nonzero valid host port; ambiguity/missing/non-loopback fails ready.

Health accepts an argv vector of 1..=64 nonempty NUL-free strings, total <=8192
bytes, timeout_ms in 1..=30000. No implicit shell expansion, stdin, TTY, injected
environment, privileged exec or output attachment. Existing image environment
applies. An explicitly configured shell in argv is still a user command, never
silently introduced. Sequence: exact container inspection -> POST exec create
(201, exact exec ID) -> POST exec start (200, Detach=true,Tty=false) -> bounded
GET exec inspection until Running=false -> fresh container inspection. Inspect
must match ContainerID and exec ID; integer exit code zero alone passes, nonzero
fails health, missing/unknown exit is unknown. Detached output is not requested
or buffered. Any lost create/start response, timeout or cancellation is unknown;
no repeat exec or replay on recovery, and health unknown is never readiness.
The exec can outlive adapter cancellation: retain the lease and report this fact;
explicit container cleanup is required to terminate remaining processes. Empty
healthcheck is allowed and means running state plus valid port, not application
health. A nonzero health result is a definitive startup failure in this first
slice, not an implicit retry loop.

## Restart limits

Changed daemon identity/connection failure retains the lease, never rebinds it.
Docker daemon ID can survive a restart; matching ID/version does NOT prove that
no restart occurred. The implementation must not advertise restart detection from
these fields. Real restart tests must measure actual retained-ID/state behavior
and uncertainty handling on an isolated daemon. A stronger epoch attestation is
outside this protocol's present guarantee; no claim of complete restart detection.
