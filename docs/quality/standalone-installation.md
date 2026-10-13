# Standalone Rust plugin installation

Core and the native Rust collector have independent versions. The collector
retains the old engine and ships as a compiled executable; matching Rust/LLVM and
Python are external dependencies. The stable engine remains an explicit candidate.

## Publication status

Core `0.4.7` selects native `0.1.0-rc.7`, which is published for all four
supported platforms. Core 0.4.7 is also published and publicly verified; see the release record. RC7 adds dependency-attribution caching and signed monorepo
source-prefix binding. See the [verified release record](../release-status.md)
and [independent candidate package guide](../releases/0.4.7.zh-CN.md).

```bash
curl --fail --show-error --location --proto '=https' --tlsv1.2 \
  -o /tmp/harness-gate-install.sh \
  https://raw.githubusercontent.com/musutrade/Harness-Gate/v0.4.7/install.sh
bash /tmp/harness-gate-install.sh --version v0.4.7 --with-rust \
  --rust-version 0.1.0-rc.7
```

Core alone uses `--version v0.4.7`. To install only the native plugin, use
`--rust-only --rust-version 0.1.0-rc.7`; no Core version is required. These
commands use Bash (Git Bash on Windows). No default Rust toolchain is changed.

## Versions, platforms and destinations

`--version` selects an exact Core `vX.Y.Z` tag. `--rust-version` accepts an exact
plugin version without the tag prefix; for example, `0.1.0-rc.7` downloads from
`rust-collector-v0.1.0-rc.7`. `HARNESS_GATE_RUST_VERSION` can set the same explicit
version; the command-line option takes precedence. There is no latest-release
lookup or fallback to another platform, engine or historical runtime bundle.

| Host | Native release asset |
| --- | --- |
| Linux x86_64 | `harness-gate-rust-collector-linux-amd64` |
| macOS Intel | `harness-gate-rust-collector-macos-amd64` |
| macOS Apple Silicon | `harness-gate-rust-collector-macos-arm64` |
| Windows x86_64 | `harness-gate-rust-collector-windows-amd64.exe` |

RC7 passed release acceptance on these four hosts. This does not certify an
arbitrary host ABI. Missing release assets fail installation.

Both programs default to `~/.local/bin`. `--install-dir` changes that private
absolute directory. The plugin is named `harness-gate-rust-collector` (with `.exe`
on Windows). For an existing `--rust-root DIR` invocation, only the plugin binary
is installed at `DIR/bin`; no runtime, environment selector or rollback state is
created. Existing bundled installations are left in place. Configure the intended
executable explicitly if an older launcher is also on PATH.

## Verification and external tools

The installer checks the signed `SHA256SUMS` and the executable's own signature.
Certificates must name the exact selected tag and its release workflow:
`release.yml` for Core, `native-collector-release.yml` for the plugin. Both use
the GitHub Actions OIDC issuer. It provisions checksum-pinned cosign 3.1.3 if no
verifier is on PATH; `--cache-dir` selects an existing pinned verifier cache.
The plugin and its signatures are verified before a combined install replaces
Core. Each executable is then replaced atomically with the existing private
path and symlink checks. Installing the pair is not a two-file transaction:
an installation-time filesystem failure can still require retrying one program.

Installation needs neither Python nor a Rust toolchain. Collection requires
Python 3.12+, the matching Rust 1.97.1 sysroot/compiler libraries and LLVM 22.1.6,
as specified by the [native runtime contract](native-external-toolchain.md).
Once those external dependencies are installed, check them with:

```bash
harness-gate-rust-collector doctor --sysroot /absolute/external/rust-1.97.1
```

A successful install verifies the executable, not the host's measurement
capability or a project's quality result. Core configuration must authenticate the
new executable digest and explicit tool paths. Existing evidence, series and
baselines require the native contract's continuity checks; installation does not
accept them or reset history.

`--offline` archives from the historical bundled product are rejected with an
explicit diagnostic. Use their immutable [historical installer and offline
instructions](rust-collector-installation.md) to restore that product. The new
installer does not unpack old archives, migrate installations or remove their
resources. `--from-source` continues to build only Core from its exact tag;
`--from-source --with-rust` installs the plugin as a signed release binary.
