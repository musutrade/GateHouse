# Release and install ownership

This note assigns release and install machinery to one owning repository for the
core/plugin split (OpenSpec task 3.3). The initial task classified ownership;
subsequent publication handoffs are recorded below. Repository naming and wider compatibility-path cleanup remain
separate work.

| Owner | Path | Why |
| --- | --- | --- |
| GateHouse | `.github/workflows/release.yml` | Core tag release workflow: build, quality gates, publish and crates.io |
| GateHouse | `install.sh` | Core install entry for `harness-gate` |
| GateProbes | `tools/release/bootstrap_collector.sh`, `build_collector_bootstrap.py` | Collector bootstrap/toolchain delivery |
| GateProbes | `tools/release/collector_*.py` | Collector lifecycle: assets, components, transport, store, receipts, policy, Sigstore, install and maintenance |
| GateProbes | `tools/release/install_collector.py`, `download_collector_release.py`, `friendly_collector_install.py` | Collector install, download and user-facing selection |
| GateProbes | `tools/release/package_independent_collectors.py`, `prepare_collector_candidate.py`, `stage_collector_candidate.py`, `sign_collector_candidate.py`, `production_collector_release.py` | Collector packaging, candidate staging/signing and production release |
| GateProbes | `tools/release/production_installer.py`, `build_user_installer.py`, `publish_user_installer.py` | Rust collector production bootstrap, user installer build and publication |
| GateProbes | `tools/release/rust-collector-release.production.yml` | Collector release workflow template |

## Shared tools

`release_policy.py`, `release_inventory.py` and `generate-sbom.py` are used by
both core and plugin release paths. They keep **one editable source in
GateHouse**. GateProbes carries generated copies from a pinned GateHouse
revision and never edits its own copy independently. The `generated_copies` map
in [`release-ownership.json`](release-ownership.json) records each copy's
repository, revision and exact digest. `release_policy.py` and
`release_inventory.py` were copied at revision `dcc9194d` for the native
collector release chain; `generate-sbom.py` is fetched at release time from the
pinned GateHouse source commit and needs no committed copy.

## Plugin release identity

GateProbes declares its own repository, tag prefix, workflow path and environment
rules. GateHouse trusts a signing identity that is explicitly provisioned, never
one inferred from a substituted download URL.

The native Rust/MIR collector now publishes from GateProbes. Its prerelease
`rust-collector-v0.1.0-rc.8` carries the Sigstore certificate identity
`https://github.com/musutrade/GateProbes/.github/workflows/native-collector-release.yml@refs/tags/rust-collector-v0.1.0-rc.8`
with matching provenance, re-verified independently. `install.sh` provisions
that identity for the Rust collector (default `0.1.0-rc.8`) while Core binary
installs through v0.4.8 keep the Harness-Gate `release.yml` identity.
Core versions starting at v0.4.9 use the GateHouse `release.yml` identity under
the explicit installer version cutover; historical tags are not copied or
re-signed. A new download URL still
never changes a trusted identity by itself. The protected production chain
keeps its separate RSA/Sigstore trust and remains tasks 4.3.

## Four independent package releases

TypeScript risk, HTTP/JSON contract, Rust source risk and Rust measurer now
publish through GateProbes `collector-release.yml`. Their signed public releases
and fresh installation acceptance are recorded in the
[publication handoff](independent-plugin-release-acceptance.md). Core `release.yml`
no longer invokes `package_independent_collectors.py` or includes their packages
in its inventory. The historical helper remains for compatibility tests and
rollback; keeping its source does not make it an active Core publication path.
Shared policy/inventory tools still have the single editable source described
above. The existing Core consumption pin is unchanged.

## Remaining collector workflows

The split leaves collector-related workflows in GateHouse with a recorded
owner and disposition; the RC8 native plugin chain and the legacy six-asset
production chain are separate products and are recorded separately.

| Workflow | Owner | Disposition |
| --- | --- | --- |
| `native-collector-release.yml` | GateHouse | Retired from active workflows on 2026-10-12: archived at `retired/workflows/`, restore commit `6aaa2342`; current main no longer triggers it. Tag creation is not prohibited, and a tag pointing at a historical commit still runs that commit's workflow. Historical releases, signatures and tags are retained. |
| `rust-collector-release.yml` | GateHouse | Paused legacy bundled-runtime RSA/Sigstore six-asset chain; not migrated, not provisioned, and not the RC8 native chain. |
| `rust-measure-native.yml` | GateHouse | Retired from active workflows on 2026-10-12: archived at `retired/workflows/`, restore commit `4ec65a76`. It built the plugin analyzer from core source; core acceptance now uses the pinned Linux amd64 measurer release and cross-platform analyzer builds belong to GateProbes. |

The paused production chain is not a prerequisite for the RC8 native delivery
and is not re-enabled by this split. Its hold step suspends signing and
publication; any revival requires a reviewed measurement migration and an
explicit product decision. No RSA key, self-hosted runner group, environment
approval or publication path is provisioned while it stays paused. GateProbes
owns the scripts and any future revival.

## Enforcement

`tools/quality/tests/test_release_ownership.py` checks that every top-level
`tools/release` file has exactly one owner, that GateHouse keeps
`release.yml`/`install.sh`, that every shared tool's editable source is
GateHouse, that each recorded generated copy still matches the digest of the
GateHouse source it was copied from, and that every workflow under
`.github/workflows/` has exactly one recorded owner and an allowed
disposition. `tools/release/tests` follows the scripts it covers and is not
classified here.
