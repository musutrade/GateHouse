# Standalone Rust plugin installation

Core and the native Rust collector have independent versions. The collector
retains the old engine and ships as a compiled executable; matching Rust/LLVM and
Python are external dependencies. The stable engine remains an explicit candidate.

## Publication status

### Historical tagged Core 0.4.8 installer

The installer frozen in Core `0.4.8` selects Harness-Gate native `0.1.0-rc.7`.
Use the immutable tag when reproducing that release:

```bash
curl --fail --show-error --location --proto '=https' --tlsv1.2 \
  -o /tmp/harness-gate-install.sh \
  https://raw.githubusercontent.com/musutrade/Harness-Gate/v0.4.8/install.sh
bash /tmp/harness-gate-install.sh --version v0.4.8 --with-rust \
  --rust-version 0.1.0-rc.7
```

Core alone uses `--version v0.4.8`; the historical plugin alone uses
`--rust-only --rust-version 0.1.0-rc.7` with that same tagged installer.

### Current independent installer

GateHouse installs Core only. Plugin flags now fail before downloads with migration
guidance. GateProbes owns the sole active native install/upgrade/rollback entry.
Download and checksum-verify its fixed source, then select the plugin explicitly:

~~~bash
curl --fail --show-error --location --proto '=https' --tlsv1.2 \
  -o /tmp/gateprobes-install-native.sh \
  https://raw.githubusercontent.com/musutrade/GateProbes/1507805205944229950e0fe57d4fe1f17fc3e130/tools/install/install-native.sh
printf '%s  %s\n' 317e7f98e3863d44d32640106091596e227be0795463dd81adcd26ef6797bce6 /tmp/gateprobes-install-native.sh | sha256sum --check
bash /tmp/gateprobes-install-native.sh --version 0.1.0-rc.8 \
  --install-dir "$HOME/.local/bin"
~~~

Provide a trusted Cosign on PATH first. On macOS use shasum -a 256 --check
for the script digest. Missing dependencies are diagnosed; the plugin installer
never provisions Rust, LLVM, Python or Cosign. Core and plugin are installed
separately. --rust-root DIR becomes plugin --install-dir DIR/bin;
HARNESS_GATE_RUST_VERSION becomes explicit --version; --cache-dir becomes
an explicitly provisioned verifier on PATH. Historical --offline bundles use
their historical tagged installer.

See the [fixed plugin installer source](https://github.com/musutrade/GateProbes/blob/1507805205944229950e0fe57d4fe1f17fc3e130/tools/install/install-native.sh)
and its [maintained installation, trust and rollback guide](https://github.com/musutrade/GateProbes/blob/main/docs/native-installation.md)
([中文](https://github.com/musutrade/GateProbes/blob/main/docs/native-installation.zh-CN.md)).
The script verifies both manifest and binary signatures against GateProbes'
exact selected native workflow/tag and GitHub OIDC issuer, with no cross-identity
fallback. RC8 is legacy mutable; its original signed manifest is additionally
checksum-pinned. Installing a binary does not adopt a series or baseline.

Core's bridge execution exit condition is fulfilled: only diagnostic stubs
remain. Historical Core installers stay unchanged, including their original
repository, version and signer. No new Core or plugin release is announced.
Protocol, measurements and Engineering Policy remain unchanged; Core owns
requiredness, thresholds, baseline/debt/ratchet, final decisions and publication.
See the [native external-toolchain contract](native-external-toolchain.md) before
collecting measurements or changing project executable/tool pins.
