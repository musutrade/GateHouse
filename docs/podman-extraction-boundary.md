# Podman extraction boundary — working contract

Status: development draft, pending independent engine-contract review. Docker's
published lifecycle profile remains unchanged. No Podman implementation or real
Podman compatibility is claimed by this document.

## Current boundary

Core 0.10 still implements Podman engine calls in
`tools/harness-gate/src/service/legacy_cli.rs`: availability, image inspection,
container start, mapped port, health exec, ownership inspection and stop. Lease,
recovery, cancellation and cleanup decisions are separate Core responsibilities.

The Docker lifecycle path is intentionally Docker-only in three places: protocol
binding validation, installation product validation, and public service/config
selection. Relaxing one check would leave the other checks inconsistent and could
confuse engine authority. A Podman binary must not impersonate `docker-lifecycle`
or use a Docker engine binding.

## Required invariants

- Select an explicit installed Podman product and engine binding. No ambient CLI
  fallback, socket default, environment-derived identity or automatic profile choice.
- Core retains durable reservation, slot/fence, exact invocation, lease, recovery,
  cancellation and operator authorization. Adapter operations remain exact-ID bound.
- Resolve image before Create through the durable reservation. Never replay an
  ambiguous mutation. Never discover a destructive target by container name.
- Keep the old Podman CLI path until the replacement is usable and tested; retire
  it with an explicit legacy-lease fence. Never migrate or silently drop old leases.
- Bind installation, config and executable bytes to each invocation. Do not infer
  Podman daemon identity from a Docker-compatible response alone.
- Distinguish exact-route absence from unavailable/unknown. A generic 404, transport
  failure, timeout or changed engine observation cannot release a lease.
- Define create body, port mapping, health exec, stop/remove policies and response
  checks against the selected Podman API profile before enabling mutation.

## Next implementation slice

First establish profile and identity validation with fake socket vectors, including
cross-engine rejection. Select versioned wire/install/config schemas only after
reviewing Podman's stable identity and API semantics. A shared transport or state
machine may be reused where its assumptions are identical; engine-specific request
and response semantics require explicit implementations and tests.

The independent review must enumerate identity/version observations, image IDs and
RepoDigests, exact create/inspect/port semantics, detached health execution,
stop/remove responses, route-specific absence, and restart uncertainty. This work
does not require installing or operating Podman. Real Podman acceptance is a later
separate step after the fake-engine implementation passes.

## Acceptance boundary

Published Docker Core0.10 + lifecycle0.1 + importer0.3 have separate real Docker29.9.2
evidence for one normal and one failed-health/operator-cleanup case. That evidence
is non-rootless, contains no restart, and provides no Podman compatibility claim.

## Implemented internal checkpoint

`runtime-protocol::lifecycle_v2::profile` now validates the exact tuple
`podman-lifecycle` / `0.1.0` / `gate-runtime/lifecycle-v2` / `podman` and a proposed
slot identity binding installation/config/executable SHA256 values. Unknown or
missing fields, mixed Docker tuples, malformed hashes and changed identities are
rejected. Equality grants no mutation or recovery authority.

This pure module is not connected to service routing or on-disk slots. It does
not parse, adopt or migrate Docker journals. Its presence in the development
crate is not a published protocol version: publication requires a new crate/Core
version after complete wire, identity, host, storage and adapter integration.

## Canonical encoding checkpoint

`lifecycle_v2::encoding` defines a separate
`gate-runtime/podman-slot-identity-v1` envelope with `schema` and `identity`.
Decode is bounded to 4096 bytes and first deserializes strict typed fields, so
missing, unknown and duplicate keys are rejected. Accepted bytes must exactly
match compact UTF-8 JSON with lexicographically ordered keys and no newline.
Historical slot documents cannot select this schema.

`match_snapshots` compares the three recorded hashes with the exact supplied
installation/config/executable bytes. It never reserializes descriptor inputs.
This is byte equality only: held-file ownership, permissions, schema validation,
provenance and post-call revalidation remain Core integration gates. It grants no
execution or recovery authority and writes no slot or journal.

The canonical writer now uses explicit ordered serialization structs at all three
levels, without `serde_json::Value` or `Map`. Field order therefore does not depend
on `serde_json/preserve_order`. A complete literal-byte regression fixes the
entire envelope representation; extra top-level and identity-level fields have
explicit negative vectors in addition to typed duplicate/missing-field tests.

## Native observation proposal (source-pinned, not live-engine proof)

The first observation profile selects native Libpod API **6.1.4** and exact server
version **6.1.4**, with no compat fallback or automatic API negotiation. Podman's
[v6.1.4 version source](https://github.com/podman-container-tools/podman/blob/v6.1.4/version/version.go)
defines native minimum 4.0.0; accepting that entire range is not this profile's
policy. The selected info route is `/v6.1.4/libpod/info`.

The [compat info handler](https://github.com/podman-container-tools/podman/blob/v6.1.4/pkg/api/handlers/compat/info.go)
creates a random UUID for each response. It cannot be used as a stable daemon ID.
The [native info types](https://github.com/podman-container-tools/podman/blob/v6.1.4/libpod/define/info.go)
provide host/store/version observations rather than a stable daemon-instance ID.

The new pure `observation` types keep protected `operator_epoch` and principal UID
outside engine observations. The epoch is an explicit 128-bit lowercase hex
operator assertion; it is neither authentication nor evidence of restart. Exact
before/after observations bind hostname, rootless, graph/run roots, selected API,
server version and local socket path/device/inode/UID/full mode. Optional advertised
remote socket data, when present, must exist and match the selected path. Its
presence or absence must remain stable across the operation.

These are normalized protocol data, not an HTTP parser. The adapter must obtain
native fields and local socket metadata independently; Core must verify protected
configuration and held files. Lexically absolute paths do not prove filesystem
canonicality. Equal observations can survive socket activation or restart and
must never be presented as continuity proof. A changed context or observation
returns a refusal for callers to retain uncertainty; it cannot establish absence.
Create/status/absence/health bodies, exact native decoding, mutation authorization
and actual slot persistence remain unimplemented gates.

## Native info body decoder checkpoint

The decoder is pinned to the v6.1.4
[native info handler](https://github.com/podman-container-tools/podman/blob/v6.1.4/pkg/api/handlers/libpod/info.go)
and [version JSON types](https://github.com/podman-container-tools/podman/blob/v6.1.4/libpod/define/version.go).
Version keys are `APIVersion` and `Version`, inside the lowercase `version` object.
It requires host hostname/security.rootless and store graphRoot/runRoot, bounds
input to 1 MiB, and rejects duplicate keys recursively before field extraction.
Additional native metrics are accepted, including finite floating-point metrics;
they do not participate in context identity. Compat/fabricated top-level identity
authority is refused. Advertised remote socket data must match the separate local
socket observation. This decoder does not establish HTTP status, route, framing,
actual socket metadata or file protection: callers must validate those separately.

`ExpectedContext::match_current_operation` is the Unix integration entry that
compares principal_uid to actual `geteuid()` before comparing bound context and
observations. The pure comparator remains available for data validation only.
Neither entry authenticates protected config; host-held config and endpoint
verification remain necessary before any operation is dispatched.

### Native inspect checkpoint (92)

`lifecycle_v2::native_status` checks GET against the exact v6.1.4 native
full-ID inspect route and bounds response JSON to 1 MiB using the shared
recursive duplicate-key decoder. HTTP 200 requires the same full lowercase ID
and selected state fields, with consistent running/paused flags. Unsupported
states fail closed. HTTP 404 requires the exact three-field native ErrorModel,
`no such container` cause, response code 404, and SQLite lookup text naming the
requested full ID. Ambiguous lookup, generic/Docker errors, other routes and
other HTTP statuses do not establish absence. This intentionally supports only
the pinned SQLite lookup error spelling; alternative storage errors fail closed.

Sources pinned to Podman v6.1.4:

- [Native route](https://github.com/podman-container-tools/podman/blob/v6.1.4/pkg/api/server/register_containers.go)
- [GetContainer handler](https://github.com/podman-container-tools/podman/blob/v6.1.4/pkg/api/handlers/libpod/containers.go)
- [SQLite lookup](https://github.com/podman-container-tools/podman/blob/v6.1.4/libpod/sqlite_state.go)
- [Error mapping](https://github.com/podman-container-tools/podman/blob/v6.1.4/pkg/api/handlers/utils/errors.go)
- [Inspect state construction](https://github.com/podman-container-tools/podman/blob/v6.1.4/libpod/container_inspect.go)

This is a pure selected-state parser, not ownership or lease-release evidence.
It does not bind image/labels/ports, protected config, held socket, observations,
a journal, HTTP framing or deadlines. These remain required before integration.
No adapter transport, mutation, real Podman operation or publication is enabled.

State spelling is pinned to [ContainerStatus.String](https://github.com/podman-container-tools/podman/blob/v6.1.4/libpod/define/containerstate.go): Podman Configured emits `created`; OCI Created emits `initialized`. Both are present, non-running observations. The impossible `configured` spelling is rejected. No lifecycle journal state is inferred by this pure parser.

### Protected context filesystem checkpoint (93, Linux only)

`ProtectedContext` loads the exact `ExpectedContext` JSON from a protected
regular file (current euid, one link, mode 0400/0600, at most 1 MiB). Recursive
JSON duplicates are rejected. It retains the config descriptor and bytes, checks
actual euid, and holds the configured socket inode using O_PATH without connecting.
The actual socket type/device/inode/owner/mode must equal the expected observation.
Config and socket paths must be canonical without symlinks; ancestors must be
root/current-user owned and not group/world writable. Revalidation checks both
held descriptors and current paths, including timestamps, owner and permissions.
Snapshots expose the same validated config bytes and their SHA256.

This checkpoint is not installation/slot authority or connected endpoint proof.
The transport must still bind its connection to the selected endpoint, enforce
deadlines and pair before/after native observations. Same-UID malicious rollback,
filesystem races during future connect, authentication and daemon restart proof
are not solved by holding O_PATH. No real engine is contacted by these tests.

### Protected read-only composition (94, Linux)

`ProtectedContext::inspect_with` composes GET info, exact full-ID inspect and GET
info using an injected exchange. It revalidates protected inputs before and after
each call, matches the first observation before inspecting, and withholds both
Present and Missing until the second observation matches and final revalidation
succeeds. Invalid IDs fail before any exchange. Errors do not trigger retries.
The callback must enforce HTTP framing, bounded reads, cancellation and deadlines;
this composition does not provide those transport properties or peer identity.

The supported local policy is explicit: rootful sockets are root-owned; rootless
sockets belong to the configured non-root principal. Protected loading enforces
this policy and current euid. This is a restricted product policy, not a claim
about every Podman deployment, user namespaces, authentication or restart safety.
The fake exchange tests do not establish real rootless or engine acceptance.
Installation, request generation, slot/lease authority and ownership fields remain
unconnected; a returned Missing is still not lease-release authorization.

### Native exec wire checkpoint (95)

`native_exec` freezes bounded explicit argv and deterministic exec-create/start
bodies, full-ID native routes, 201 ID responses and detached-start empty 200.
Container-create response validation requires an explicit empty Warnings array;
null/missing/nonempty warnings fail closed. Container create request network/port
policy and follow-up ownership inspection are not implemented in this checkpoint.

Health inspection binds exact exec/container IDs, command arguments, user,
privilege, TTY and attachment flags. Completed requires `Running=false` AND
`CanRemove=true`: Podman's newly created, unstarted exec also has false Running
and zero ExitCode. Pending is not successful health. A terminal exit code is
preserved (0..255), including nonzero failure. No mutation is sent by these helpers.
There is no idempotency key: the future host/adapter must durably reserve sends,
retain unknown outcomes, never retry create/start, and bind deadline/cancellation
and before/after context. Pure decoding is not a durable no-replay guarantee.

Pinned source:

- [Exec handlers](https://github.com/podman-container-tools/podman/blob/v6.1.4/pkg/api/handlers/compat/exec.go)
- [Exec inspect construction](https://github.com/podman-container-tools/podman/blob/v6.1.4/libpod/container_exec.go)
- [Exec inspect JSON fields](https://github.com/podman-container-tools/podman/blob/v6.1.4/libpod/define/container_inspect.go)
- [Exec create fields](https://github.com/podman-container-tools/podman/blob/v6.1.4/vendor/github.com/moby/moby/api/types/container/exec_create_request.go)
