# Rust collector installation and independent release

> Delivery amendment (2026-09-14): this page describes the existing immutable
> bundled release. New development retains the old native engine in a
> [standalone binary with external dependencies](native-external-toolchain.md),
> using Core's release process. That product is not yet signed or published.
> Stable remains a candidate. The old bundled publication workflow stays suspended.
>
> Separation record (2026-10-12): this page's six-asset RSA/Sigstore production
> chain is **paused and unprovisioned** — its workflow hold suspends signing and
> publication, and no RSA key, runner group or publication path is configured.
> The separate RC8 native Rust/MIR external plugin is now signed and published
> from GateProbes with a keyless identity, documented in
> [native-external-toolchain.md](native-external-toolchain.md) and installed
> with `install.sh --rust-only`. The two products have separate trust and
> lifecycle; neither status change re-enables or replaces the other.

For the **unreleased GH-255 source implementation**, see the
[lightweight lifecycle guide](lightweight-collector-installation.md). Its component
store, archive-free verification and maintenance commands supersede the source
lifecycle details below; the immutable published installer described here is unchanged.

The collector **0.1.0-rc.3** and compressed installer **0.1.0-rc.4** are published.
See [current release receipts and compatibility](../release-status.md). This guide
supersedes the historical GH-229/GH-231 preparation instructions for normal use.
The RC3 compatibility matrix includes Core 0.4.2 on the exact tested Linux
x86_64 host. The original native capture and identical runtime payload are
reused explicitly; the new manifest, installation and Core binding are verified.

## Recommended installation

Use the immutable installer revision below for Core plus Rust. The v0.4.2 source tag predates this RC3 plugin delivery and retains its older Rust installer pin. Existing tags are not rewritten.

Download the current Core installer from its immutable source revision:

```bash
curl --fail --show-error --location --proto '=https' --tlsv1.2 \
  https://raw.githubusercontent.com/musutrade/Harness-Gate/9ffa2b829ec25ca54fcb00a9bc720284c3913924/install.sh \
  -o /tmp/harness-gate-install.sh
# Add only the optional plugin
bash /tmp/harness-gate-install.sh --rust-only
# Or install Core 0.4.2 and the plugin together
bash /tmp/harness-gate-install.sh --version v0.4.2 --with-rust
```

The root script pins the child installer's SHA-256. The child pins its private
Python bootstrap, transport catalog, RSA public key, Sigstore trusted root and
cosign v3.1.3 digest. It provisions these inputs automatically and checks the
original RSA and Sigstore signatures before installing the reconstructed archive.
The initial shell installer and host OS remain the trust entry point; a plugin
cannot supply its own replacement verifier or trusted key. No system Python or
Harness-Gate source checkout is needed for this installation path.

Required host tools are Bash, curl, sha256sum, tar/gzip and the pinned host
OpenSSL/runtime libraries. The released ABI is Linux x86_64, glibc 2.43, kernel
`7.0.0-31-generic`, with exact library digests, not general Linux support.
Unsupported hosts reject before the large toolchain download.

Default root: `~/.local/share/harness-gate/rust-collector`.
Default cache: `~/.cache/harness-gate/collector`.
Use `--rust-root DIR` and `--cache-dir DIR` on the Core installer; the standalone
`install-rust.sh` calls the root option `--root DIR`.
The installer prints the exact versioned launcher. `current.json` records selection;
there is no automatic collector executable on PATH. Keep captures outside the
installation tree. Installation never changes the default rustup toolchain,
creates project signing keys, accepts a baseline or replaces existing project gates.

The two compressed layers total 282 MB; first use with bootstrap, verifier and
metadata totals approximately 444 MB. Verified unchanged layers and cosign are
cached. Interrupted large downloads retain completed ranges; rerun the same
command to resume. Reinstalling an existing version verifies it and selects it.
The original signed archive and extracted runtime are retained, so disk usage
is larger than the compressed download size.

## Offline installation

On a connected machine, download **all twelve assets** from the
[installer release](https://github.com/musutrade/Harness-Gate/releases/tag/rust-collector-installer-v0.1.0-rc.4)
into one directory. Add the official
[cosign-linux-amd64 v3.1.3](https://github.com/sigstore/cosign/releases/download/v3.1.3/cosign-linux-amd64)
with that exact filename and SHA-256
`4629c757b7618056f8ddd7e2625ae9fdd94c0372a65049520bc7d9df9efc7f71`.
Also retain the authenticated Core `install.sh` downloaded above. Transfer this
kit intact, then on the supported offline host run:

```bash
bash /path/to/install.sh --rust-only --offline /absolute/path/to/offline-kit
```

The twelve assets include `install-rust.sh`, its Sigstore bundle,
`installer-bootstrap.tar.gz`, `installer-build.json`, `transport.json`, both
compressed layers and the five original control/metadata files. There is no
need to download the 1.06 GB `collector.tar` for this path. The offline flag is
for Rust installation only; it does not implement offline Core installation.
The installer still verifies every input and the reconstructed signed payload.

## Original signed assets and production trust

The [signed RC3 release](https://github.com/musutrade/Harness-Gate/releases/tag/rust-collector-v0.1.0-rc.3)
retains exactly six assets: `collector.tar`, `manifest.json`, `sbom.spdx.json`,
`provenance.json`, `release-inventory.json`, and `release-inventory.sig`.
The production signature control contains RSA and Sigstore verification material;
legacy RSA-only v1 trust is insufficient for this release. The production RSA
key is 3072 bits (384-byte signatures). Public trust is carried by the separately
authenticated installer, and the private key stays in the protected environment.
See the [six-asset readback receipt](release-0.4.2/published-verification.json).

Checks bind the entire inventory, payload hashes, source, provenance, exact ABI,
and signer identity. Missing, extra, tampered or incompatible inputs fail closed.
[Immutable source, notices and relink materials](https://github.com/musutrade/Harness-Gate/releases/tag/rust-collector-materials-7165558-v1)
and [per-file engineering review](https://github.com/musutrade/Harness-Gate/releases/tag/rust-collector-review-7165558-v1)
remain applicable to the identical runtime payload; [RC3 incremental review](https://github.com/musutrade/Harness-Gate/releases/tag/rust-collector-materials-718bc5a-v1) binds the new manifest and Core tuple. Installer and plugin versions remain independent.

## Advanced lifecycle

For operator-managed lifecycle operations, use the authenticated source version
of `tools/release/install_collector.py`, its same-source imported modules, host
Python 3.11+ and an independently pinned **v2** trust file. This advanced entry
is distinct from the automatic shell installer. For example:

```bash
python3 tools/release/install_collector.py --root /absolute/collector-root --trust /absolute/host/trust-v2.json select --version 0.1.0-rc.3
# Roll back to the previously installed RC2, retained after upgrade
python3 tools/release/install_collector.py --root /absolute/collector-root --trust /absolute/host/trust-v2.json rollback --version 0.1.0-rc.2
python3 tools/release/install_collector.py --root /absolute/collector-root --trust /absolute/host/trust-v2.json recover
python3 tools/release/install_collector.py --root /absolute/collector-root --trust /absolute/host/trust-v2.json uninstall --version 0.1.0-rc.3
```

Install and select print the exact version's launcher path. `current.json` is the
atomic selection record; it is not an executable shim or a change to global PATH.
P6 connects the selected path through existing generic configuration. No command
installs rustup, changes global tools, collects evidence, or accepts a baseline.

The installer locks a private root, journals owned paths, snapshots the fixed
assets into private staging, then verifies signature, inventory, provenance and
host ABI before manually extracting. It rejects traversal, links, devices,
sparse/PAX metadata, duplicate/missing/extra entries and unsafe modes. It checks
each extracted payload digest and the complete tree before renaming the version
directory and atomically replacing the selection record, with fsync at durable
boundaries. Selection/rollback reverify signatures, ABI, every installed byte and
file mode. An existing version is never overwritten. The full signed assets
remain under each version's `.delivery` directory for offline re-verification;
budget roughly archive plus extracted runtime storage per version, plus staging
for the next install. No size or performance claim is established by tiny fixtures.

Before version commit, interruption leaves the previous selection unchanged;
recovery removes only journal-owned staging. A crash after version rename can
leave a complete unselected version, which explicit selection can reverify and
activate. A crash after pointer replacement leaves the new complete version
selected. Recover is idempotent, including interrupted uninstall. Uninstall
removes only a verified version and its retained delivery files; if active, it
first clears selection. It does not automatically select a fallback version.

Unexpected files or links inside an owned tree stop cleanup without deleting
them. Project captures, original binaries, anchors and unrelated files are never
cleanup targets. Preserve the journal and unexpected bytes for operator review;
move unrelated evidence to a safe project location before retrying recovery.
The private root assumes trusted same-UID administrators and a local filesystem
with atomic rename/flock/fsync semantics; it does not defend against a malicious
root user or certify network filesystems. Tests include actual SIGKILL and
injected interruption boundaries; they are not hardware power-loss tests.

Distribution authentication is not runtime capture authentication. Version or
path relocation establishes no measurement-series equivalence. Released Rust
Core retains requiredness, thresholds, lineage, debt/ratchet and final outcomes;
all accepted coverage/CRAP rules and capability states remain unchanged.

## Publication and historical evidence

The existing `.github/workflows/rust-collector-release.yml` is active. Its
separate operations cover rehearsal, private signing, original RC publication
and compressed installer publication. The dedicated `rust-collector-release`
environment requires the pinned owner's manual approval, allows self-review
under the explicitly accepted single-maintainer v2 policy, disables admin bypass,
and permits only main. Core publication keeps its separate `release` environment.
Exact-source main CI, approved packet digests and immutable version rules remain.
Stable collector promotion still waits for separately accepted GH-215 integration.

Historical [GH-229 fixture validation](gh-229/validation.md),
[GH-231 preparation](gh-231/preparation.md) and
[GH-239 governance proposal](gh-239/operator-preparation.md) describe their dated
scope; their old pending notes are not the current release status. Synthetic
local rehearsals remain tests and never stand in for production signatures.

The stable Rust candidate remains in its separate development branch and has
explicit limitations. It is not yet an installable replacement.
