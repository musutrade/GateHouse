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

## Revision 3: reconciliation and evidence boundary

This section supersedes revision 2 where it maps all known-ID crashes to retained.
Removing and awaiting-absence recover to `reconciling-remove`, preserving object
ID and the original durable remove request digest. Failed absence verification
also preserves this state. Reconciliation may issue a fresh read-only exact-ID
status request, but never another remove. A verified absence transitions to
verified-absent; present (including running), mismatch or unknown leaves it locked.
This includes crashes before dispatch, after dispatch, after a success response,
after the removal claim is saved, and during absence verification. A crash before
dispatch can leave a present container requiring operator intervention; safety
must not be traded for an automatic replay.

The pure state module is NOT the durable lease layer or evidence validator. Its
Event values may only be used in production by the forthcoming journal wrapper.
That wrapper must validate the protected expected request, full response, exact
pending request digest/revision, installation/config/engine binding and target
before constructing an event. An adapter cannot deserialize an Event. The wrapper
must reject reused responses, wrong ID/daemon, stale generation and wrong operation
before invoking transition. State-model tests do not prove those checks exist.
Do not connect this module directly to the process host.

The journal adds `remove_request_sha256` and `absence_evidence` (both explicit
null until established). Absence evidence contains status_request_sha256,
remove_request_sha256, object_id, exact_route, http_status (404),
error_message_sha256, observed_before and observed_after. The adapter's validated
response supplies these route facts; Core also reconstructs the exact route and
expected message from the full target ID. This remains trust in the pinned adapter,
not independent proof from an adversarial daemon. Save evidence and verified-absent
in one atomic fsynced transition, then remove the active marker and fsync its
parent. Keep a protected tombstone containing evidence. A crash before marker
removal resumes marker cleanup from that tombstone; a missing marker without a
valid verified-absent tombstone never grants reacquisition. Reacquisition uses a
fresh lease invocation under the resource lock and preserves the old tombstone.

Startup failure returns a failing service result with static classification
`lifecycle-retained`, resource ID, lease path and known object ID (or explicit
unknown). It does not return a RunningService whose destructor invokes cleanup.
It schedules no cleanup, retry or background task. Operator-requested Core cleanup
is the only subsequent mutation path; unresolved-no-id has no mutation path.

## Revision 3: operation matrix

All successful facts below require matching before/after GET /version and GET
/v{api}/info observations. Every request is one bounded invocation. `unknown` and
`rejected-before-effect` are the only shared failure outcomes; both have exactly
`kind` and a static `reason` enum (invalid-input, identity-mismatch, transport,
deadline, cancelled, invalid-response, unsupported-state). Observations on these
failures are explicit null. Unknown means effects may have occurred; rejected is
permitted only if no effect-capable HTTP request was dispatched. Both retain the
lease. A nonzero process exit or invalid envelope is host-level unknown, regardless
of any JSON. Valid envelopes, including known failures, exit zero.

| Operation | Only nonfailure outcome | HTTP and body requirements | Core action |
| --- | --- | --- | --- |
| resolve-image | image | GET /images/{encoded exact ref}/json, 200; Id is sha256: plus 64 lowercase hex, RepoDigests unique strings; requested digest reference must match RepoDigests | persist image ID before pending-create; no pull |
| create | created | POST /containers/create?name={encoded name}, 201, full 64-hex Id, Warnings null or empty; fresh exact-ID inspect 200 matching name, image ID and labels | durably store ID, never start on create claim alone |
| status | status or absent | GET /containers/{full-id}/json, 200 with exact ID/name/labels and known state; absent only bounded exact-route 404 described above | read proof; no mutation implied |
| start | started | POST /containers/{id}/start, 204 or 304; fresh inspect 200 running matching target | starting -> running |
| stop | stopped | inspect first; created/exited no-op, running POST /containers/{id}/stop?t={grace} 204/304 then inspect created/exited | stopping -> stopped |
| remove | removed | inspect created/exited, DELETE /containers/{id}?force=false&v=false, 204 | awaiting-absence only |
| health-exec | health | sequence below; exit_code required signed i32, exec and container identities match | zero -> ready, nonzero -> retained failure |

Resolve-image is read-only but uses the same protected binding. Core acquires the
resource lock, allocates the new lease invocation and writes an exclusive
`gatehouse-lifecycle-reservation/v1` reservation before resolution. Reservation
contains project_id,resource_id,lease_invocation and expected request digest. It
blocks concurrent acquisition; crash recovery performs no automatic resolution
or create. Resolution success creates and fsyncs the complete pending-create lease
before removing the reservation. A resolution failure retains the reservation
for explicit abandonment (no container mutation has occurred). Create uses only
the pinned image ID, never the tag; another tag change cannot change that request.
A resolve-image 404 is unknown/unavailable, never container absence.

Health uses POST /containers/{id}/exec -> 201 with exactly Id (full 64 lowercase
hex), then POST /exec/{id}/start -> 200 with empty body for detached execution.
Exec create body has Cmd, AttachStdin=false, AttachStdout=false, AttachStderr=false,
Tty=false, Privileged=false; no Env/User/WorkingDir overrides. Start body is
Detach=true,Tty=false. GET /exec/{id}/json -> 200 requires exact ID, ContainerID,
Running boolean and integer ExitCode when Running=false. Polling interval is
100ms, bounded by the original monotonic deadline, never a fresh timeout per poll.
Any missing/wrong ID, unknown exit, non-200/201 or lost response is unknown and
retained. Container identity is checked before exec and after completion. Every
poll remains read-only; no second exec create/start is allowed. Nonzero exit is
returned in health, not converted to unknown. Tests must cover cancellation before
create, after create, after start and during polling, plus mismatched ContainerID.

## Revision 3: canonical bytes

Use only UTF-8 JSON objects, arrays, strings, booleans, null and integers within
signed i64 or unsigned u64 bounds. Reject floats, negative zero, exponent forms,
invalid Unicode, duplicate keys and unknown fields before canonicalization.
Sort object keys by unsigned UTF-8 bytes. Preserve array order; labels/environment
must already be sorted uniquely by key. Emit integers in minimal decimal (no plus
or leading zeros), booleans/null lowercase, no whitespace or trailing newline.
Strings escape quote and backslash, use \b, \t, \n, \f, \r for those controls,
and lowercase \u00xx for other U+0000..001F controls; emit all other Unicode as
UTF-8 without normalization or slash escaping. Field validators reject NUL where
applicable. This is the project's integer-only canonical JSON profile, not a
claim of arbitrary JSON canonicalization support.

To calculate operation_key, OMIT binding.operation_key entirely (never null),
canonicalize the remaining complete request and prefix the exact ASCII domain
`gate-runtime/lifecycle-v1/request` followed by one NUL byte. SHA256 is lowercase
hex. Insert this value, canonicalize again; these complete bytes are both fd4 and
stdin. Each must independently pass strict schema validation and canonical-byte
comparison; then compare the two byte strings exactly, including operation_key.
Recompute operation_key and require the response echo it, generation and the full
binding/operation exactly. No whitespace-tolerant equality or field omission is
allowed at this boundary. Generation is the durable lease revision reserved for
this invocation; increment under lock and persist before dispatch. Read-only calls
also reserve a revision, preventing response reuse across reconciliations.

Canonicalization primitive test vector (deliberately not a schema-valid lifecycle
request; full request vectors will accompany wire types):

```json
{"binding":{"generation":7},"operation":{"kind":"status"},"protocol":"gate-runtime/lifecycle-v1"}
```

Domain-prefixed SHA256 with omitted operation_key: `53c04196d0ecf86a07b1e5cb1244d3170f0b21fd1cc565394683c1c833f350e9`.
The full-schema/fd4 tests must also reject whitespace variation, null operation_key,
stale generation, duplicate fields, changed target and a matching echo with the
wrong digest. This document does not claim those wire validators are implemented.

## Revision 4: explicit operator reconciliation

Under the same exclusive resource lock, a new operator cleanup invocation reserves
and fsyncs a strictly greater generation before obtaining fresh exact-ID status.
From reconciling-remove, only this new authorization plus fresh matching running
status permits stopping, or matching created/exited status permits stopped ->
new nonforced remove. The old request is never resent. Ordinary recovery/status
cannot produce the operator events. Preserve previous remove digests in an
append-only bounded attempt history; reject additional attempts if full rather
than discarding evidence. Each attempt records its new generation/nonce/request
and links the old digest. Persistence failure prevents dispatch.

Retained known-ID leases may be released by a new operator reconciliation status
that proves exact-ID absence; no remove claim is synthesized. Its tombstone records
reason `operator-observed-absence`, with remove_request_sha256 null if no removal
was dispatched. For post-remove absence use `post-remove-absence` and the durable
remove digest. Both require matching installation/config/daemon/target, a fresh
status request digest, and evidence-before-marker-removal durability. Unknown,
wrong ID/daemon, unresolved-no-id and pending-create cannot take either release
path. New operator authorization is explicit action, not inferred from expiry,
start failure, recovery or a status response. Existing global authorization to
execute tests does not stand in for this runtime journal authorization token.

State-only events model this distinction but do not verify authorization; the
journal wrapper must construct them only after validating those fresh facts.

## Implemented wire surface (development only)

`runtime-protocol/src/lifecycle/wire.rs` now defines request/response types and
bounded canonical parsers. `Object` additionally requires `image_id` so the create
result can be checked against the immutable requested image. `absent` carries an
`evidence` object with object_id, route, http_status, error_message_sha256; its
before/after observations remain on the response envelope. `removed` has only
kind and is never treated as absence. `observations` must be present, null only
for unknown/rejected-before-effect. Missing RepoDigests is invalid; an empty list
is valid only for a non-digest-pinned image request. All nonfailure operations
require matching observations. Health nonzero exit is a validated health outcome,
not success/readiness. The caller must interpret exit_code; wire validation alone
does not select a successful state transition.

The independent Python-generated complete status request is checked in as
`runtime-protocol/src/lifecycle/wire/status-request.json` (no trailing newline).
Tests compare Rust's domain hash and complete canonical bytes against that file,
exercise protected-input equality and response binding, and cross-check every
successful operation/outcome pair. Typed roundtrip equality rejects fields that
Serde's tagged unit variants otherwise silently discard. The initial failing
extra-field test is retained in external evidence.

These parsers verify bytes and their expected binding, not fd4 file ownership,
process exit, network route execution, monotonic deadlines or replay consumption.
Those remain host/adapter/journal responsibilities and are NOT implemented here.
In particular, accepting the same response twice against the same supplied
expected request is possible until the durable wrapper consumes the generation.
No host or engine is connected to these types.

## Exact Docker create and port policy

Create emits POST /v{api}/containers/create?name={encoded name} with JSON keys:
Image = immutable image_id; Labels = unique requested labels; Env = sorted
`key=value` entries; ExposedPorts = {"{container_port}/tcp": {}};
AttachStdin=false, AttachStdout=false, AttachStderr=false, OpenStdin=false,
StdinOnce=false, Tty=false; HostConfig = {AutoRemove:false, Privileged:false,
PublishAllPorts:false, NetworkMode:"bridge", RestartPolicy:{Name:"no",
MaximumRetryCount:0}, PortBindings:{"{container_port}/tcp":[{HostIp:"127.0.0.1",
HostPort:""}]}}. No host binds, device mappings, extra capabilities, host namespace,
volume deletion, command/entrypoint override or pull. Image-declared configuration
still applies (including volumes and entrypoint); this is not an image sandbox.
The empty HostPort requests allocation by Docker. Image/label/port policy must be
verified against fresh inspection after create/start; ignoring surplus port
bindings is prohibited. Any unsupported configuration fails closed.

Every exact-ID Target now includes nonzero container_port, saved in the lease and
bound into the request digest. A response may contain zero port bindings (a fact
usable for cleanup) or exactly one binding for that same TCP port on 127.0.0.1
with a nonzero host port. Additional/wrong-port/non-loopback/UDP/ambiguous bindings
are invalid; never select the first binding. Started and health require exactly
one matching binding. Status with no binding is not service readiness; retaining
this observation lets Core safely clean a container that failed port allocation.
Create/stop may legitimately have no assigned port. No lifecycle adapter exists
yet; these HTTP create fields are the specification for its future implementation.

## Journal implementation checkpoint

`lifecycle/journal` now implements a distinct record with immutable create request,
state, revision, pending request/purpose, bounded complete attempt history and
absence response. Full requests retain operation digests, nonces, endpoint pins
and every remove intent. This concrete record supersedes the earlier illustrative
flat field list; no historical lease reader is reused. Root is one resource's
protected directory for this lease, provisioned explicitly by Core. There is no
automatic directory discovery, archival, second-lease acquisition or resolve-image
reservation implementation yet.

On Unix the guard opens/locks a protected directory, reads only regular 0600
single-link members using descriptor-relative no-follow opens, and rejects
unprotected ancestors. Active marker is fsynced before initial lease creation;
a marker without a valid record blocks reuse. Request intents are committed via
exclusive temporary file, file fsync, renameat and directory fsync before a
non-Clone borrowed Dispatch is returned. Failed commit poisons the guard. Dropping
a token cannot authorize another dispatch. Reopen consumes unresolved pending
requests as unknown and never issues an old token. Nonces and generations must
be fresh; successful mutations require a fresh in-guard status observation.

OperatorCleanup is an explicit trusted Core policy input, not human authentication
and not something an adapter can request. Known-ID recovery requires explicit
reconciliation/operator purpose. Retained absence requires operator purpose;
post-remove absence also permits read-only reconciliation. All returned objects
must retain the original immutable image ID. Attempts are bounded at 128 and
record bytes at 65536; exhaustion fails closed without discarding history.

Verified absence is committed with its complete request/response first. Release
then commits an identical tombstone, removes active, and fsyncs the directory.
Record and tombstone are retained. Each crash window can resume release without
another mutation. Missing marker without matching durable absence/tombstone is
rejected. Directory locking is cooperative; no protection against a hostile same
UID or rollback of an entire protected directory is claimed. Tests inject I/O
failures; they are not power-loss/hardware durability certification.

This checkpoint does not implement the Core command policy, full normal teardown,
resource reacquisition, installed host verifier, fd4 process transport or an
adapter. Dispatch::observe must only be called after host process checks; bytes
alone cannot prove process exit success. A valid lifecycle record alone must never
be used to bypass these remaining integration requirements.

## Protected installation/input implementation checkpoint

`lifecycle/installation` loads the exact host/installation/config schemas, verifies
all three canonical documents and external installation/config/executable SHA256
pins, rejects old protocols/public expected paths, checks protected ancestors,
owner/mode/single-link regular files, and holds descriptors with complete file
identity revalidation (including inode, ctime and mtime). The new product identity
is `docker-lifecycle` version `0.1.0`; this is an unreleased implementation target,
not a change to old runtime/readiness artifacts. Executable bytes are bounded at
64 MiB. A request must match the held digests, complete engine config and current
principal before a host can use it. These checks do not pin dynamic libraries or
prove a daemon's authentication/containment.

Linux adapter-input validation requires both fd3 config and fd4 expected input to
be private 0600 unlinked regular memory files with all F_SEAL_WRITE/GROW/SHRINK/SEAL
flags, at most 65536 bytes each. This specifies kernel write-sealed memfds (as used
by Core's existing SealedInputs), not necessarily an O_RDONLY open-file flag.
Pread avoids shared-cursor assumptions. Expected and stdin are exact canonical
matches; config digest and all engine fields must match that request. Ordinary
files, unsealed/public/empty/oversized descriptors are rejected. Seals prove byte
immutability, not that a trusted Core created the descriptor: the future host must
combine the held installation and journal Dispatch when creating these inputs.

No process-spawn path yet consumes this new installation snapshot or Dispatch.
The Core host, native lifecycle adapter, service config/lease orchestration and
real Docker validation remain required. The current tests use never-executed
fixture executables and memfd descriptors; they are not daemon tests.

## Journal and installation review repairs

The persisted record is replayed from its immutable create request through every
attempt and explicit recovery boundary before it can authorize another operation.
Stored phase, purpose, target, generation and absence proof must equal the replayed
record. Recovery boundaries invalidate earlier live status observations. The
128-attempt limit rejects additional attempts without truncating history.

ServiceCleanup authorizes normal stop/remove only for a successfully running
service and its successful normal-stop continuation, with fresh exact-ID status.
A failed or cancelled cleanup remains retained and requires operator authority.
Live Removing/AwaitingAbsence status that finds the container present enters
ReconcilingRemove without requiring the journal to be closed and reopened. A
dropped dispatch can be durably consumed as unknown via retain_unknown; this
never replays the operation. Explicit operator reconciliation can then clean up.

Absence proofs record the exact status operation digest and distinguish operator
observed absence from post-remove absence, including the latter's remove request
digest. The durable tombstone carries this proof before the active marker is
released. Marker-only interrupted creation remains blocked, not rediscovered.

VerifiedInstallation exposes held config bytes and engine binding only after
revalidating every held file identity. The process host must use these bytes for
fd3 instead of reopening an ambient path. Same-byte replacement of host,
installation, config or executable invalidates the snapshot.

## Core process host implementation checkpoint

The Linux lifecycle host consumes a journal Dispatch and a VerifiedInstallation;
it has no entry accepting a freely supplied executable or public expected file.
It uses the revalidated held config bytes as sealed fd3, the token's exact canonical
request as sealed fd4 and stdin, clears the environment and uses `/` as cwd. It
requires ELF and executes a duplicate held executable descriptor through
`/proc/self/fd` while retaining it across spawn; the duplicate is above fd4 and
close-on-exec. Dynamic libraries are not pinned and this is not OS containment.

The existing bounded process supervisor supplies timeout, cancellation, process
group termination and output limits. Nonzero exit, any stderr, incomplete writes,
invalid response, changed installation or cancellation/deadline after validation
consume the attempt as unknown. Valid responses enter the journal only after those
checks; journal persistence failure remains an error. There is no automatic retry,
CLI fallback, discovery or name-only operation.

Host tests execute a freshly compiled local ELF fixture that checks sealed fd3/fd4,
exact expected/stdin equality, empty environment and cwd. It returns no container
identity and cannot contact Docker. Service configuration, identity acquisition,
lease orchestration and the lifecycle adapter still need integration; this host
checkpoint is not a claim that Docker lifecycle has left Core or passed real
engine acceptance.

The Core-facing host returns explicit `DispatchError::Unknown` and
`DispatchError::RejectedBeforeEffect` after persisting those envelopes, and
`DispatchError::Boundary` after persisting a transport/boundary failure as unknown.
These outcomes cannot be mistaken for `Ok(Response)`. Even an accepted nonfailure
response is operation-specific: Created does not mean Started, Status does not
mean healthy, and Removed does not mean absence or lease release. Service
orchestration must match the operation/outcome and durable phase explicitly.

The host requires Linux procfs `/proc/self/fd` access; descriptor execution failure
is consumed as unknown with no fallback. Deterministic private observer tests now
prove that a valid Created response commits its exact ID, and that cancellation
or same-byte host/installation/config/executable replacement after input creation
or after response validation prevents that commit. The production observer is a
no-op; tests do not depend on sleeps or guessed callback counts to place the final
cancellation boundary.

## Paired native adapter regression

The opt-in `lifecycle-joint-tests` Cargo feature requires
`GATEHOUSE_LIFECYCLE_ADAPTER` and `GATEHOUSE_LIFECYCLE_ADAPTER_SHA256`; missing inputs
or a mismatched artifact fail the test instead of skipping it. The test copies the
pinned executable into a protected installation, then runs durable ResolveImage, Create, Status, Start, another Status and HealthExec generations through the actual Core host, sealed descriptors, native adapter
and a fake Unix HTTP socket. It verifies operation-specific outcomes and the exact
journal identity; it performs no real Docker operation. Default tests need no
external repository artifact.

Additional host tests cancel at prepared inputs, exhaust the deadline only after
a validated response, and alter the journal marker/lease at both host boundaries.
A journal commit failure never returns success or makes the consumed dispatch
reusable; damaged journal state remains an explicit recovery problem. These are
process/filesystem regression tests, not real daemon/rootless/restart evidence.


## Durable image reservation checkpoint

Service image resolution uses `Reservation::create` with the exact ResolveImage
request and immutable CreatePlan. The protected directory lock and durable active
marker are acquired before any resolution dispatch. The one-use ResolutionDispatch
is accepted by the same sealed-input Core host as journal operations. A validated
Image response is persisted before Create can be authorized; unknown, rejection,
transport failure or interrupted pending resolution cannot automatically retry.

The reservation transfers its exact evidence and plan into a generation-2 Create
journal using a fresh invocation. The journal validates the resolution proof during
history replay and retains the reservation's active marker. It persists lease.json
before returning the Create token. Crashes before or after that transfer cannot
produce a second automatic Create. A reopened resolved reservation requires an
explicit trusted operator resume decision; ordinary recovery must not authorize it.
A pre-Create abandonment persists its reason and tombstone before releasing the
active marker, and retains its records. Once a lease exists, reservation abandonment
and resolution dispatch are prohibited; journal recovery owns the resource.

The older direct generation-1 Create journal API remains for callers that already
hold an exact image identity and existing tests. The service integration must use
the reservation path when resolving a configured image reference. No old lease or
environment-derived authority is implicitly migrated.

The paired native regression covers this transfer and a nonzero HealthExec result:
exit 7 is valid health evidence but leaves the journal Retained, never healthy.
Transfer filesystem failure tests retain conservative recovery evidence. These
checks do not yet connect the public Service lifecycle, publish the new protocol,
or establish real Docker, rootless or daemon restart behavior.


## Service coordinator checkpoint

The Linux `LifecycleService::prepare` entry binds a configured image, Core-owned
resource identity, immutable CreatePlan, health argv, protected resource directory
and held installation into a private PreparedService. It constructs a fresh 128-bit invocation
using kernel entropy and obtains engine identity/digests only from a revalidated
installation snapshot. Its start path consumes the prepared slot and creates a Resolution reservation;
it cannot adopt a free Request or direct generation-1 journal. After resolution
it requires the evidence-bound generation-2 Create, fresh Status before Start,
and fresh Status before optional HealthExec. A nonzero health exit fails startup
and leaves the durable state retained. There is no automatic replay or Drop cleanup.

Explicit normal cleanup requests fresh status, Stop without force, another status,
Remove without force/volumes, then a new exact-ID absence status. Only the journal's
absence proof permits tombstone/release. Any failure leaves durable records.
Recovery is deliberately not exposed as normal cleanup or automatic resume.

The native joint tests cover failed-health retention and successful health followed
by normal teardown against fake Unix HTTP. Public configuration routing, protected
state-directory provisioning, ServiceManager ownership and explicit operator
recovery still need connection before this coordinator replaces the legacy path.


## Protected service slot checkpoint

`ServiceSlot` holds an exclusive lock on a preprovisioned protected resource
root and persists a strict canonical `gatehouse-lifecycle-service-slot/v1` record.
The immutable record binds the exact resolution request (installation/config/
engine/project/resource/invocation/image), CreatePlan and health argv to a fence.
Health arguments are validated before claiming the slot. A private PreparedService
owns this slot plus the held installation; start consumes it and accepts no separate
identity, image, plan, health argv or journal root. The slot creates the only allowed
per-invocation reservation directory. Slot identity/bytes are rechecked before and
after host calls. Dropping a prepared/start-failed service leaves its fence active.

Normal cleanup commits journal absence/tombstone before marking the slot released.
A later acquisition verifies the previous exact reserved journal and tombstone,
archives the prior slot record, increments the fence and requires a new invocation
directory. An unresolved slot cannot be reacquired even when its process has died.
A crash before slot publication has no dispatch capability; a published slot with
missing child journal is retained. A crash after journal release but before slot
release remains fenced for explicit recovery. Normal acquisition never performs operator recovery. As with the journal, this is cooperative same-user protection,
not anti-rollback protection against a hostile same-UID writer.

The coordinator requires Created status before Start, Running before Health, and
Created/Exited after Stop before Remove. It requires exactly one TCP loopback port
matching the immutable plan. Exited services, failed startup, uncertain stop or
wrong absence remain operator-recovery cases. Public config routing must derive
the resource root from one protected state location/project/resource mapping;
it must not expose arbitrary per-start roots or adopt legacy lease-v2 records.
That routing and explicit recovery remain integration gates.


## Mapped roots and explicit known-ID recovery checkpoint

Core preparation now uses a resource directory derived from the protected state
base and a length-delimited SHA256 of project/resource identity. Image, installation
path, configuration and invocation changes do not change that resource directory.
The allocator holds the base lock while provisioning a private child, rejects
symlinks/unprotected children, and then holds the resource lock. The public Core
configuration route must still supply one fixed Core-owned state base; arbitrary
per-start bases are not a supported public interface.

`OperatorRecovery::open_mapped` is a separate trusted Core operator entry. It requires
exact project/resource, current fence, lease invocation and a fresh 128-bit operator
authorization identity, persists an intent before dispatch, and reopens only the
exact reservation-derived journal. Its token can issue only Status, Stop and Remove;
no resolution, Create, Start, Health or name discovery is permitted. Core validates
the recorded request against the held installation before dispatch. A verified
absence produces a durable audit proof bound to the slot and journal digests before
publishing slot release. A partial-release crash (journal tombstone present, slot
still unreleased) can be completed explicitly without an engine call.

Failed health/known-ID startup can be explicitly stopped and removed after fresh
status and exact absence. Unknown-ID Create and pre-Create reservation failures
remain fenced: this checkpoint intentionally has no operator name-discovery or
pre-Create abandonment entry. The API expresses trusted Core policy, not human
authentication. Public operator CLI/configuration routing and those remaining
recovery modes are still gates; normal startup never calls this entry.


Directory-lock guards explicitly unlock when their owning process ends the guard,
then close the descriptor. CLOEXEC alone is insufficient when another thread forks:
a blocked child can inherit the open file description and prolong flock after the
parent closes it. The guard records its owner PID so a child's destructor cannot
unlock a still-live parent's guard. A deterministic blocked-child regression retains
the original failure and verifies the fix; the durable slot fence still prevents
reacquisition after failure even though the process lock is released.


## Public managed-service routing checkpoint

On Linux, `services.<id>.lifecycle_host` selects a protected lifecycle installation
explicitly. Core derives the state base as the canonical project root plus
`.harness-gate/lifecycle-services`, and the resource as `service:<id>` using the
project input identity. No user-selected state base or environment-selected host
is accepted. Existing group/world-writable ancestors, symlinks and a nonprivate
state directory are rejected without permission repair. Immutable loaded service
configuration supplies image, environment, one container port and health command;
Core mints the invocation and ownership labels before acquiring the resource slot.
Failed startup keeps the durable slot. ServiceManager teardown uses the lifecycle
host and exact-ID absence proof; neither startup nor cleanup falls back to CLI.

`external_env` cannot accompany `lifecycle_host`. Availability remains a separate
read-only `readiness_host` operation; without that explicit host, availability
returns an error rather than interpreting lifecycle installation as readiness.
Other operating systems reject lifecycle service startup. Services without the new
field keep the existing legacy engine route until subsequent migration. The public
operator command, versioned protocol publication and real Docker acceptance remain
gates; this checkpoint does not claim that all service engine calls are extracted.

If a lock owner dies without running Drop, a surviving fork child can retain its
open file description. Policy is conservative: refuse lock acquisition until all
inherited holders close/exec/exit. Never force-unlock, clear the durable fence or
start a second container. A subprocess regression tests actual owner `_exit` and a
surviving blocked child. This is an availability limitation, not proof of daemon
or process containment.

An operator audit proof is persisted before service-slot release. If release fails,
that proof does not mean the service slot is free; the slot record remains the
release authority. Retrying explicit recovery requires a fresh authorization
identity; the prior intent remains replay-blocked. Already released slots reject
operator recovery instead of creating another intent.


## Explicit operator command

`harness-gate lifecycle-cleanup --service <id> --fence <current-fence>
--lease-invocation <recorded-invocation>` is a separate operator action. It uses the
same fixed project/resource mapping and the service's explicit lifecycle host.
Core mints a fresh 128-bit audit authorization. The current fence and invocation
must be supplied exactly; configuration never infers them or grants authority to a
new slot. The command permits only known-ID Status/Stop/Remove and exact absence
release, including completion of a previously journal-released slot. Normal
startup, doctor, generic cleanup and cancellation never invoke this command.
A changed installation cannot clean up a journal bound to another installation.
Unknown-ID and pre-Create states remain retained; no name discovery or automatic
start retry is added. This command does not establish external user authentication.


Recovery-boundary regression coverage now injects failure immediately after the
operator proof is durably published and before slot release. It verifies the
unchanged unreleased slot, journal tombstone and removed active marker, refusal of
normal reacquisition, refusal of the old authorization, and successful fresh
explicit authorization that completes release without any dispatch. The production
path supplies a no-op boundary observer; no persistence checks or thresholds change.
Readiness success validates the selected readiness installation only. Even when
both host fields are set, it does not validate lifecycle installation or prove that
lifecycle startup will succeed.
