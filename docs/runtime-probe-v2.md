# Linux runtime probe v2 — Core 0.7.0 candidate

`harness-gate adapter runtime-probe-v2 --request actual.json --trusted-host /protected/host.json`

This additive command exposes the read-only native host. It is not in released
Core v0.6.0. It does not select the service engine, migrate leases or authorize
mutation. Remove and Absent remain refused; Inspect returns Present or Unavailable.
The actual request is untrusted and must equal the separately pinned expected
request. No pins are inferred from environment, working directory or actual input.

The host document has exactly this shape (digests are lowercase SHA-256 values):

```json
{
  "schema": "gate-runtime-host/v2",
  "executable": {"path": "/protected/adapter", "sha256": "<binary SHA-256>", "version": "0.0.0"},
  "config": {"path": "/protected/config.json", "sha256": "<raw file SHA-256>", "schema": "gate-runtime-engine-config/v2"},
  "expected": {"path": "/protected/expected.json", "sha256": "<raw file SHA-256>", "schema": "gate-runtime/v2"},
  "cwd": "/protected/working-directory",
  "timeout_ms": 5000
}
```

Host, config and expected files must be canonical absolute paths, regular files
owned by the invoking euid, mode exactly 0400 or 0600 and link count one. Ancestor
directories must belong to root or that euid and be neither group- nor world-
writable; sticky directories are not an exception. Symlink paths, hardlinked
leaves and FIFOs are refused. Each read is capped at 65536 bytes. Held-file and
named-file identities are checked around reads and execution; unexpected changes
prevent a response from being returned. The untrusted actual file is only required
to be a bounded regular no-follow input; it is not a trust source.

Config and expected SHA-256 pins cover exact bytes. Executable digest/version
must match the expected request. ELF format is checked on the held executable;
version is trusted metadata, not queried from the program. This does not bind
loader/libraries or eliminate pathname-execution races. Cwd must be canonical;
there is no claim that these checks isolate hostile code. Protected ancestors and
cooperative local filesystem behavior remain assumptions; filesystem reads have
no hard timeout. Socket checks and daemon observations belong to the adapter and
are assertions, not cryptographic daemon/rootless/restart attestation.

The command returns one verified v2 response only after completion checks. Static
errors do not echo input contents or adapter stderr. Fake-socket tests are not
real Docker compatibility evidence. No published native runtime artifact or
production recovery/OS sandbox claim is made by this command.

The loader checks ancestor paths before opening/revalidating leaves; it does not
hold every ancestor directory fd. Same-euid/root adversarial path races are outside
this local trust model. Cwd is checked for canonicality, not protected ownership
or permissions. Executable ancestry is not protected by this loader; its existing
single-file digest and identity checks do not provide full path isolation.
The owner-mismatch integration fixture requires root and returns without exercising
chown when run unprivileged; report that case as unexercised, not owner acceptance.
