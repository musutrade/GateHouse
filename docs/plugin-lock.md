# Explicit plugin onboarding (v1)

Core accepts an explicitly selected **data-only preset catalog** through one
generic format. Names belong to publishers; Rust/Python and future ecosystem
names require no Core enumeration. GateProbes owns product metadata, preparation,
installation, upgrade and rollback. Core owns compatibility, signature and file
verification. This interface does not install or execute adapters; adapter v2
continues to require complete host/operator-signed requests and replay protection.

Commit `.harness-gate/plugins.lock.json` in the project. Each v1 entry binds
`name`, `source`, exact numeric `major.minor.patch` `version`, the lowercase
`manifest_sha256` (without a prefix), and `signer`. There is no floating version,
discovery, registry lookup, environment selection, or background update.

```json
{
  "schema": "harness-plugin-lock/v1",
  "plugins": [{
    "name": "publisher-defined",
    "source": "https://example.org/reviewed-product",
    "version": "1.0.0",
    "manifest_sha256": "REPLACE_WITH_64_LOWERCASE_HEX_DIGITS",
    "signer": "operator-reviewed"
  }]
}
```

Schemas: [lock](../schema/plugin-lock.schema.json),
[manifest](../schema/plugin-manifest.schema.json). A store contains
`NAME/VERSION/plugin.json`, `plugin.sig`, and the manifest's inventoried files.
The manifest declares `schema = harness-plugin/v1`, identical name/source/version,
`core_versions` (exact Core package versions), `protocol = harness-preset-catalog/v1`,
`platforms` (`linux-x86_64`, `macos-aarch64`, etc.), `presets` (catalog directory),
`files` (relative path → raw SHA-256), `programs` (plain PATH names), and
`dependencies` (selected plugin name → exact version). Every catalog reference
must exist in the signed file inventory. All files are regular, confined,
symlink-free, at most 1 MiB each; packages contain at most 128 files, and locks
select 1..64 unique names. No installation hooks or permission declarations exist.

`plugin.sig` is `{"key_id":"operator-reviewed","signature":"BASE64"}`.
The Ed25519 signature covers the concatenation of UTF-8
`harness-gate/plugin-manifest/v1`, **one NUL byte**, and the **exact raw manifest
bytes**. JSON reformatting changes the digest and signature. Duplicate JSON keys
are rejected. `verify_strict` rejects malformed or invalid signatures.

The operator explicitly supplies public key JSON files with `key_id` and a
base64 32-byte `public_key` using `--plugin-trusted-key PATH` (repeatable).
Neither a lock nor a manifest can add trusted keys or capabilities. The CLI
selection is the authority; keep host keys outside the checkout and review them
independently. Artifact signing does not grant request-signing authority or
enable collectors. Existing quality trusted-state, request signatures, capability
restrictions and nonce ledgers remain required whenever quality is configured.

```sh
harness-gate --plugin-store /host/plugins --plugin-trusted-key /host/operator.json \
  plugins --lock /project/.harness-gate/plugins.lock.json --json
harness-gate --project-root /project --plugin-store /host/plugins \
  --plugin-trusted-key /host/operator.json init --plugin publisher-defined --preset preset-name
harness-gate --project-root /project --plugin-store /host/plugins \
  --plugin-trusted-key /host/operator.json config check --format json
harness-gate --project-root /project --plugin-store /host/plugins \
  --plugin-trusted-key /host/operator.json doctor --json
harness-gate --project-root /project --plugin-store /host/plugins \
  --plugin-trusted-key /host/operator.json verify --all
```

Init copies verified bytes into a private view before resolving references.
Configuration checks and doctor expose missing items and next actions. Verify,
hook, step, compatibility verification and plan reject invalid locks before
dispatching gates/collectors/services. Staged verification and plans read the
**indexed lock**, including its attributes; working-tree pins cannot substitute
for it. Preflight reads local prepared files and PATH metadata without running
dependency programs, fetching network data, or consuming signed requests.
The lock pins preset provenance; generated flow/quality configuration remains
project-owned and independently validated. It is not an executable sandbox or a
replacement for adapter request authentication.

Stable diagnostic IDs appear in the existing config report envelope and in
doctor details:

| ID | Repair |
| --- | --- |
| `HGPLUGIN-LOCK` / `HGPLUGIN-SELECTION` | Review v1 pins; explicitly select a locked catalog at init. |
| `HGPLUGIN-STORE` / `HGPLUGIN-MISSING` | Explicitly prepare/install exact offline packages or missing files. |
| `HGPLUGIN-VERSION` / `HGPLUGIN-CORE` | Select matching source/version and compatible Core; no automatic upgrades. |
| `HGPLUGIN-PROTOCOL` / `HGPLUGIN-PLATFORM` | Prepare a compatible reviewed package for this host. |
| `HGPLUGIN-DIGEST` | Restore exact bytes; investigate corruption before reviewing any upgrade. |
| `HGPLUGIN-SIGNATURE` | Operator verifies identity, key and signature outside project configuration. |
| `HGPLUGIN-DEPENDENCY` | Explicitly pin dependencies or install existing toolchains yourself. |
| `HGPLUGIN-PATH` / `HGPLUGIN-LIMIT` / `HGPLUGIN-MANIFEST` / `HGPLUGIN-CATALOG` | Repair the bounded signed inventory and catalog references. |

Migration is opt-in: projects without a lock keep existing explicit `--presets-dir`
workflows. To adopt v1, review prepared artifacts, operator keys and the install
plan, then explicitly apply it. Changing a package requires a new version and
reviewed pin; installed version directories are immutable. To roll back, restore
the prior lock using the GateProbes receipt; packages remain available for audit
and offline reuse. Removing a lock to return to legacy catalog selection is an
explicit project change, never an installer fallback. Unsupported schema versions
fail closed. Installation changes no flow, quality configuration, measurement
series, baseline, debt, ratchet, required gate or system toolchain.

See [GateProbes onboarding and real two-ecosystem acceptance](https://github.com/musutrade/GateProbes/blob/main/docs/plugin-onboarding.md).
Engineering Policy remains in Core: plugins provide facts and checks; Core keeps
requiredness, thresholds and final/release decisions. Marketplace, automatic
discovery and a resident manager are outside v1.
