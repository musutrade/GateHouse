# Lightweight collector lifecycle (GH-255, unreleased)

This describes the implementation in this source revision. It does **not** change
published RC tags, the root `install.sh` pin, or a user's current installation.
The [published installation guide](rust-collector-installation.md) still applies
to the released installer. See [local measurements and limits](collector-install-costs-20260913.md)
and [ADR 0051](../adr/0051-lightweight-collector-storage.md).

## Install and use

After the publisher ships an authenticated installer containing this implementation,
the standalone installer supports:

```bash
bash install-rust.sh --plan
bash install-rust.sh --download-policy allow
# Deterministic private environment: acquire all missing pinned bytes, ignoring PATH tools.
bash install-rust.sh --mode pinned --download-policy allow
# Reuse exact bytes from an existing runtime, copying into installer-owned storage.
bash install-rust.sh --reuse-runtime /absolute/runtime --download-policy allow
```

The default network policy is `deny`, including CI. `allow` is an explicit acquisition
policy, with the complete byte plan printed before acquisition. An uncached `--plan`
prints the private interpreter bootstrap size, all compressed components, verifier,
metadata and the complete upper bound without downloading. Once the pinned interpreter
is cached, the plan resolves exact host reuse. The installer displays both network
and offline acquisition bytes, missing objects and reasons, and root/cache locations.
It never silently switches to a weaker measurement backend.

The private interpreter is still a required bootstrap. Host rustc, LLVM, Python and
C runtime candidates are reusable only when the required file hashes match. Reused
external bytes are copied into private storage once, not linked to mutable user tools.
No rustup default, system package, project file or policy is changed. On subsequent
versions, private objects are shared by SHA-256 and file mode. This reduces repeated
storage and acquisition, but does not make a missing compiler small.

Use `harness-gate init` and the project's normal `harness-gate verify` after configuring
the printed versioned collector path as described in the published guide. Keep evidence
outside the install tree. The installer performs signature checks and a disposable
compile/run/coverage self-check automatically; it never adopts project baselines.

## Independent versions and upgrades

Core, collector delivery version and component content revisions are separate.
`component_versions` reports independent Rust/LLVM, C linker, Python, runtime-library,
license and plugin revisions. The signed manifest remains the compatibility authority:
protocol, exact Core version/commit/digest matrix, host ABI/library fingerprints,
compiler/LLVM versions and bytes, and measurement/normalization identities.
A similar version string is insufficient. Unsupported host tuples stop before runtime
acquisition; missing compatible components are identified in the plan.

Run the new authenticated installer to upgrade. Only changed or missing objects are
acquired. Preparation, original release authentication, complete payload verification
and a real compile/run/coverage diagnostic precede atomic `current.json` activation.
Failure preserves the prior selection. The launcher path printed by installation is
versioned; an existing project pinned to another path stays explicitly pinned.
By default the current version plus two previous versions remain available.

```bash
bash install-rust.sh --action rollback --version 0.1.0-rc.3
```

Rollback rechecks signed metadata, host ABI, modes and every payload byte before
selection. Core compatibility is also enforced when Core invokes the collector.
Project business dependency updates normally require recompilation/measurement only.
Compiler or measurement semantics changes must pass the existing reviewed series
transition workflow; rerun with the intended tools, inspect the reported incompatible
identity, and explicitly review a new baseline/transition under the project's policy.
Installation, rollback and Core upgrades never silently substitute a new baseline.

## Storage and migration

```bash
bash install-rust.sh --action usage
bash install-rust.sh --action migrate
bash install-rust.sh --action cleanup --keep 2 --dry-run
bash install-rust.sh --action cleanup --keep 2 --cache-limit-bytes 536870912
```

Use `--root DIR` and `--cache-dir DIR` for isolated, private (0700), distinct locations.
Maintenance automatically provisions the pinned verifier if needed; its network policy
is also explicit. Usage requires no verifier. Runtime usage separates versions,
shared objects and staging; logical byte totals may count shared files repeatedly,
while `total_allocated_bytes` counts each regular-file inode once using `st_blocks`.
Cache usage separately reports complete downloads, resumable partial/range bytes and
trust files. Directory entries, root lock/journal files and unknown cache files are
outside these totals. Cleanup reports before/after values and the actual reclaimed
regular-file allocation; dry run reports candidates and zero reclaimed bytes.

Migration recognizes existing `versions/0.1.0-rc.*` trees with `.delivery/collector.tar`.
It verifies all versions before mutation, writes and verifies a compact replay receipt,
then removes the redundant tar and shares exact private objects. An interrupted
receipt write leaves the original archive valid; an interrupted sharing operation can
be retried. Mixed legacy/receipt layouts remain supported. Rollback versions retain
their signed controls, notices and licenses. Full replay verifies the **original signed
tar digest** from installed payload bytes and retained headers without allocating a tar.
The receipt supplies no independent trust authority.

Installation, migration and cleanup serialize on private filesystem locks. Cleanup
validates retained and removable versions first, protects the current version regardless
of age, and removes only unreferenced installer-owned objects. Unknown content and
symlinks fail closed. Never run recursive manual deletion on the object store: hardlinked
objects may be required by multiple versions. The cache limit applies to complete
verified downloads; pinned active verification tools and resumable partials are exempt.
Rerunning an interrupted download verifies and resumes it, then removes its partials.
No command prunes external toolchains, unrelated cache entries or project dependencies.

## Offline modes and release automation

For zero network acquisition, use `--offline /absolute/kit` (optionally `--mode pinned`).
The publisher's offline kit must include the authenticated shell installer, its private
bootstrap, compressed objects identified by the catalog, all five signed controls and
the pinned `cosign-linux-amd64` verifier. Already verified private objects/cache entries
can satisfy missing kit entries. Offline mode still performs full signature and content
verification and the native self-check. It does not install Core offline.

Full original release export is explicit:

```bash
bash install-rust.sh --action export --version 0.1.0-rc.3 --export-output /absolute/new-export
```

The export directory must be new. The original tar is reconstructed and verified before
exposing the directory, alongside its five signed controls. This is an archival release
export; add the authenticated installer bootstrap/catalog/verifier for a standalone
lightweight offline kit. Export does not fetch publisher-only review/rebuild logs.

The existing protected publication path builds the component catalog from authenticated
release bytes, performs the native diagnostic, signs the installer containing all object
hashes and compatibility declarations, and uploads the component objects. Original dual
RSA/Sigstore release verification, SPDX, provenance, eligibility and complete license
payloads remain mandatory. CI runs the component/transaction tests; publication approval
and clean-host acceptance remain publisher responsibilities. This change does not publish
a release or claim that local RSA diagnostic material is production certification.
