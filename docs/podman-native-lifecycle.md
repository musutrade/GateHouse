# Native Podman 5.7 lifecycle

Status: the Core 0.13.0 / Podman lifecycle 0.3.0 / importer 0.6.0 predecessor
chain is published. Core CLI retirement from PR #183 is shipped in
Core 0.14.1; its downloaded Linux binary passed real rootless Podman acceptance.
See the [public verification receipt](releases/0.14.1-public-verification.md). See [current release status](release-status.md)
and [candidate migration/acceptance](releases/0.14.0.zh-CN.md). The implementation
notes and original publication checklist below retain their historical context.

## Selected profile

Core 0.13.0 reuses the durable lifecycle-v1 reservation, journal, fence, exact-ID
ownership and operator-recovery machinery for a second, explicit engine profile:

| Field | Docker profile | Podman profile |
| --- | --- | --- |
| Product/version | `docker-lifecycle` 0.1.0 | `podman-lifecycle` 0.3.0 |
| Protocol | `gate-runtime/lifecycle-v1` | `gate-runtime/lifecycle-v1` |
| Engine | `docker` | `podman` |
| API | existing Docker compat version | `5.7.0` Libpod only |
| Rootless | existing behavior | required |
| Reference | existing readiness/capture hosts | separate capture 0.2.0 profile |

`VerifiedInstallation` accepts only the exact product/version pair and requires
its engine to match the protected configuration. A Podman capture host
(`gate-runtime/podman-host-v1`), the read-only `podman-lifecycle` 0.2.0
installation, or a Docker/Podman mixed descriptor fails before a lease is
acquired. `lifecycle_host` is accepted for `runtime = "podman"` only with that
mutation tuple; `readiness_host` remains Docker v3.

## Podman 5.7 facts

- Podman 5.7.0 serves the Libpod API under `/v5.7.0/libpod/...`; there is no Docker
  daemon UUID. The protected configuration pins the observed host name
  (`daemon_id`), graph root (`scope`), exact API/server version, rootless flag,
  socket path/owner and principal UID. These observations do not authenticate the
  peer or prove that a daemon did not restart.
- Create accepts a full resolved image ID, exact name, ownership labels,
  environment, one loopback TCP mapping with `host_port: 0` (Podman allocates the
  host port) and an optional bounded `command`. It adds no pull, privilege,
  auto-remove, restart policy, mounts, devices or volume removal.
- Create returns a full ID only after exact created-state inspection. Start
  requires created and proves running. Remove is non-forced and requires a
  terminal state.
- Podman 5.7 rootless stop can return a network-cleanup error after the container
  is already terminal. The adapter accepts the stop only when a fresh exact-ID
  inspection independently proves a terminal state; otherwise the result is
  Unknown.
- Absence is only the exact container route's 404 with
  `no container with name or ID "<id>" found: no such container`.
- Health uses one detached Libpod exec, started once and polled by exact exec and
  container identity under the original deadline.

Unknown, timeout, cancellation and malformed response outcomes remain durable
journal records for explicit operator recovery. Core never automatically replays
create or start and never discovers an unknown ID by name.

## Service integration

`services.<id>` with `kind = "docker"`, `runtime = "podman"`,
`lifecycle_host = "/protected/host.json"` and an optional bounded `command` uses
the native lifecycle host for startup and teardown. `command` is rejected for
Docker and for configurations without an explicit Podman lifecycle host. The old
Core Podman CLI route is retired; migration and retained legacy lease handling are
documented in [Legacy CLI retirement](runtime-lifecycle-v1.md#legacy-cli-retirement).
Normal cleanup uses the lifecycle
journal and requires exact post-remove absence. The 15-second historical cleanup
budget was raised to 30 seconds because five real Libpod exchanges (status, stop,
status, remove, absence status) do not fit reliably in 15 seconds on a loaded
rootless host.

`harness-gate init` now protects `.harness-gate` with mode 0700. Previously a
default umask created it 0775, so the documented private-ancestor requirement
made every lifecycle service fail on a freshly initialized project.

## Local real acceptance

Linux x86_64, rootless Podman 5.7.0, pinned Alpine image
`b92841763c5c2140a20eeb2a44e94ebb0e80fd51d9d6b5895ea72fa999525d68`. A uniquely
named owned service container was created, started, health-checked, stopped,
removed and proved absent through the public `harness-gate verify` service path.
The verification result was PASS and no container remained. This is local
development evidence, not downloaded-release, signature, daemon-restart or OS
containment acceptance.

## Historical coordinated publication checklist (Core 0.13.0)

Original candidate matrix, in dependency order (superseded by the release-status entry above):

1. onboarding-presets 1.0.8: published and verified; Core CI now fetches the
   exact archive `87cd7c8107a1082d3ff3113d14d4402abd241e651d168262038f569abfd89dc1`
   and runs its 29-step acceptance. See
   `docs/releases/onboarding-presets-1.0.8-acceptance.md`.
2. protocol 0.6.0 and Core 0.13.0.
3. podman-lifecycle 0.3.0 (archive, registry lock, manifest and hashes).
4. runtime-importer 0.6.0 with the Podman 0.3.0 profile.

The lifecycle fixture keeps its historical 0.4.0/0.4.1/0.5.0 entries. The 0.6.0
entry binds the reviewed GateProbes branch `fixture/podman06-source-152` at commit
`d69c59762bce6320f3d6e98ae1843af58d09e3b5`. Its real GitHub tarball identity is
prefix `musutrade-GateProbes-d69c597/`, size 1,045,332 bytes and SHA-256
`065fa6a2ff1fc452e4464bc7bff65149289e3c99ec7764d18fe4431df14f0e16`. The remote
archive is downloaded and checked before extraction. The protocol crate 0.6.0 is
still not registered, so the fixture prepares exactly one local path override,
rejects unrelated lock changes, records the resolved lock SHA-256
`e6e5ce69fc63e7a7767a02d204a6a959c8969167e94873153aa05261eb37356c` in the
receipt, and builds every binary with `--locked`. No unpublished registry
checksum is invented. The final fixture build produced a real receipt with the
Core source digest and product binary hashes; all 27 lifecycle joint tests and
all 11 Podman capture joint tests pass. The earlier config-recognition difference
did not reproduce; the traced failure was a stale expected error string, now
fixed. Its original cause remains unproven and is not attributed to disk-full.

Core CLI contracts have been rebuilt for 0.13.0. Package and importer scripts now
select the product/importer profile from the reviewed manifests, retain the
historical 0.2.0/0.5.0 profiles and fail closed until the actual Core 0.13.0 and
Podman 0.3.0 pins/acceptance are available. Downloaded full-chain Core 0.13.0
acceptance, the real GitHub fixture mapping, CI exact counts and the remaining
fixture discrepancy are still pending. Nothing in this document authorizes a
push, tag, release or PR for Core, protocol, product or importer.
