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
