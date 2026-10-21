# Protected installed runtime probe

This candidate adds `harness-gate adapter runtime-probe-installed --request
ACTUAL --trusted-host HOST` on Linux. It uses the existing v2 read-only execution
boundary. It neither installs an adapter nor replaces service or lease execution.
It is not present in released Core 0.7.1.

The host document has the same required `executable`, `config`, `expected`, `cwd`
and `timeout_ms` fields as [the v2 host](runtime-probe-v2.md), but its schema must
be `gate-runtime-host/installed-v1`. It additionally requires `installation`:
a protected descriptor `path`, its exact byte `sha256`, and schema
`gate-runtime-installation/v1`. Missing descriptors, unknown/duplicate fields,
and v2 host documents are rejected. The old v2 command rejects the installed
schema; neither entry silently falls back to the other.

The descriptor has exactly these required fields:

| Field | Meaning |
| --- | --- |
| `schema` | `gate-runtime-installation/v1` |
| `repository` | Nonempty, control-free operator audit text |
| `tag` | Nonempty, control-free operator audit text |
| `source_commit` | 40 lowercase hexadecimal characters |
| `archive_sha256` | 64 lowercase hexadecimal characters |
| `executable` | Exact `path`, `sha256`, `version`, equal to the host pin |

The existing host checks also require the executable digest/version to equal the
trusted expected v2 request. Actual input cannot supply missing trusted pins.
The installed executable path must be absolute and canonical, with root/euid-owned
ancestors that are not group/world writable. The executable leaf must be a
single-link regular file owned by the current euid, mode 0500, 0700, 0555 or 0755
(no special bits or group/world write). Its metadata is held and revalidated
before and after execution in addition to the existing executable digest checks.
The descriptor uses the existing
bounded, euid-owned 0400/0600 single-link protected reader. Host, descriptor,
config and expected files remain held and are revalidated before and after
execution. Executable digest and file identity retain the v2 host's checks.

Provenance fields are audit assertions by the local operator, **not publisher
authentication**. Core does not fetch a release, extract an archive, verify a
release signature, or prove that `archive_sha256` describes the installed file.
A separate GateProbes offline importer must check independently reviewed release
pins and archive/inventory/SBOM contents before writing a descriptor. That importer
is not implemented by this slice. Hand-authoring a protected descriptor grants
only the same local trust as hand-authoring a protected host.

The original cooperative-filesystem boundary remains: ancestors must be protected
from untrusted writers; this is not hostile same-uid/root isolation, race-free
ancestor-fd traversal, rollback protection or an OS sandbox. Filesystem operations
have no hard deadline. Capabilities and read-only Present/Unavailable observations
are supported; Remove and Absent remain refused. No daemon attestation, rootless
proof, real-engine compatibility, service integration or lease migration is claimed.
