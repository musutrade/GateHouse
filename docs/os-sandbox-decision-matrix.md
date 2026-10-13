# OS Sandbox Decision Matrix

**Status:** Decision record for a future, not-yet-implemented sandbox
**Date:** 2026-09-03

Harness-Gate's current adapter isolation is limited to protocol allowlists,
process-group lifecycle controls, bounded readers, and evidence validation.
This matrix records what a separate platform-sandbox decision must select
before any document may claim OS-enforced isolation. It is a decision matrix
only; no platform sandbox is implemented by this record.

## Decision boundaries

1. A future sandbox SHALL be a separate ADR/OpenSpec change with per-platform
   fixtures and operational evidence; this matrix does not approve one.
2. The selected primitive SHALL define identity, network, filesystem,
   resource, and descendant behavior for each supported OS.
3. When a platform primitive is unavailable, the product SHALL fail closed:
   either reject the sandboxed invocation or run with the current bounded
   wording, never silently advertising stronger isolation.

## Platform matrix

| Dimension | Linux | macOS | Windows |
| --- | --- | --- | --- |
| Candidate primitives | Namespaces, seccomp, Landlock, cgroups v2, setrlimit | App Sandbox (Seatbelt) profile, code-signing identity, Endpoint Security for audit, setrlimit | Job Objects, AppContainer / SILO, integrity levels, per-user SID/AppContainer SID |
| Process identity | `/proc/<pid>/stat` start time; PID namespace PID | audit token / responsible process identity; `proc_pidinfo` start time | process creation `FILETIME`; Job object association |
| Network | New network namespace or Landlock/seccomp deny; loopback only unless explicitly shared | Seatbelt `network-*` sandbox extensions; no per-process network namespace | AppContainer loopback exemptions and firewall rules; no per-process network namespace in classic Job Objects |
| Filesystem | Mount namespace plus read-only/`tmpfs` roots, Landlock rules, or `chroot`; reject symlink escapes | Seatbelt `file-read*`/`file-write*` allowances; resolve `/var`/`/tmp` aliases | AppContainer profile file capabilities and integrity level; reject path escapes outside granted roots |
| Resource | cgroup CPU/memory/IO and PID limits; `setrlimit` per process | `setrlimit` for basic per-process limits; no full cgroup-style tree accounting without additional tooling | Job object CPU, memory (Windows 8.1+), process, and affinity limits |
| Descendants | PID namespace plus cgroup release; kill entire namespace with bounded cleanup | App Sandbox applies to the app/process tree; verify session/daemon escape behavior | Job object kills descendants by default; SILO adds isolation, verify behavior |
| Unavailable / unsafe fallback | Landlock or cgroups absent on older kernels; ambient namespaces unavailable | Endpoint Security and Seatbelt behavior varies by macOS version; no API for namespace-level network isolation | Job-object memory limits unavailable on older Windows; AppContainer requires supported signing/runtime constraints |
| Failure mode | Sandbox setup failure rejects the invocation and emits structured evidence | Profile load or extension failure rejects the invocation | Job/AppContainer creation failure rejects the invocation |

## Required evidence before acceptance

- One automated fixture per platform proving a denied network/filesystem/
  resource/descendant operation fails closed before or during execution.
- A compatibility note for each unavailable primitive, including the recorded
  fallback and owner.
- Rollback path that restores the current protocol/lifecycle wording without
  weakening evidence contracts.
- Cross-platform CI runs with the fixture matrix above.

## Related records

- [ADR-0038](adr/0038-post-remediation-hardening.md)
- [Adapter isolation wording inventory](adapter-isolation-wording-inventory.md)
- OpenSpec `post-remediation-hardening` tasks 1.1-1.3
