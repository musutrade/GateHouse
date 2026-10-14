# Quality presets and explicit migration

This guide describes Core 0.4.9 and current GateHouse source builds. Historical Core 0.4.8
still embeds the ecosystem presets and does not accept `--presets-dir`; use
`harness-gate init --preset rust-api` with that release. See the
[version-specific quick start](../README.md#installation-and-quick-start).

`init --presets-dir /path/to/GateProbes/presets --preset rust-api`, `angular-only`, and `angular-rust-postgres` now write
coherent flow + quality configuration, policy/series files under
`.harness-gate/packs/`, security configuration and `.harness-gate/QUALITY.md`.
Full and CI participate in quality. Hook omits expensive collection with partial
assurance; it cannot claim full-quality PASS.

| Reference preset | Quality configuration |
| --- | --- |
| `rust-api` | `app` component; accepted Rust line/region coverage >= 4/5 and required CRAP <= 30 in full/CI. |
| `angular-only` | `app` component; native TypeScript line/function coverage >= 4/5; optional CRAP diagnostic remains unsupported. |
| `angular-rust-postgres` | Frontend and backend reuse those same packs; a separate API contract pack binds breaking changes, generated-client drift and compatibility to their relationship in one project aggregate. PostgreSQL remains an execution service. |
| `generic` | Execution configuration only; quality remains opt-in. |

The [Engineering Policy](engineering-policy.md) is unchanged. Rust policy limits,
requiredness and the complete accepted retained series are preserved. Packs map
native reference scopes onto configured targets. TypeScript does not gain a
certified CRAP or branch/complexity capability through a preset. Unsupported CRAP
has no numeric measurement and cannot satisfy a required policy. Component
coverage defaults are explicit adoption policy, not claims about measured code.

## Selecting external files

Check out a reviewed GateProbes revision and select its `presets/` directory:

```sh
export HARNESS_GATE_PRESETS_DIR=/path/to/GateProbes/presets
harness-gate presets
harness-gate init --project-root /tmp/review-quality --preset rust-api
```

The directory contains `catalog.json` with schema `harness-preset-catalog/v1`,
`presets` entries (`description`, `flow`, optional `recipe`) and `packs` mapping
pack names to JSON files. Paths are relative to this directory. Core rejects
escaping paths, symbolic links, duplicate JSON keys, unknown catalog fields,
files over 1 MiB and catalogs over 64 presets or packs. A missing or invalid
explicit directory fails without falling back to embedded ecosystem templates.
The loader does not download, install or discover packages. Listing without an
external directory shows only `generic`. Released Core 0.4.8 predates this loader;
use a Core build containing the migration when following these instructions.

## Reference UX and pack composition

Reference combinations are UX, not core architecture. The generic quality schema,
compiler, collector host and verifier do not dispatch on ecosystems or on fixed
frontend/backend pairs. Go, Vue, Java, Python, React and other ecosystems can add
collectors, capabilities, policy and pack data without redesigning that model.

Presets consume declarative `*.packs.json` recipes and `packs/*.json` fragments.
Each selection names a pack and supplies bindings such as component ID and source
root. Expansion substitutes strings and keys structurally, merges object fields
and set-like arrays, and rejects conflicting values, duplicate output files,
missing bindings and escaping file destinations. Policy values and series are
opaque to the composer. The standard quality validator checks the result.
Recipes share ecosystem packs; a contract is its own capability pack, independent
of the language pair. Packs carry reference series and capability states in
`capabilities.json`; this setup metadata grants no collector trust or PASS.

The long-term direction is composable ecosystem/preset packs: a project can choose
`frontend=vue`, `backend=go`, `database=postgres`, then compose component execution,
collector/capability and policy data plus any explicit contract pack. A database
pack contributes quality only when a selected capability requires it. This avoids
bespoke `vue-go-postgres` or `react-java-postgres` logic. Ecosystem flow templates, recipes and packs now live in GateProbes. Core embeds
only `generic` and loads a reviewed directory through `--presets-dir PATH` or
`HARNESS_GATE_PRESETS_DIR`; the explicit flag takes precedence. A future registry/package
manager can supply the same fragments and composed flow configuration; registry
discovery, installation, version resolution and trust provisioning are not shipped
here. No language-specific behavior belongs in generic preset plumbing.

The synthetic `nebula-unregistered-2049` fixture composes through the same function
without a catalog entry and cross-validates against generic execution components.
Regression tests also guard generic preset plumbing, pin native reference series,
verify policy/profile boundaries and initialize/cross-check every public preset.

## Enablement and migration

Existing flow-only repositories retain their current verification semantics.
`config migrate` rejects legacy v1 database recipes with GateProbes migration help.
`gateprobes-postgres migrate` creates the new execution flow and does not add or
replace quality configuration.
`init --force` explicitly replaces generated files; it is not a migration helper.
`generic` neither creates nor deletes quality configuration.

1. Generate the chosen reference into a separate temporary directory, for example
   `harness-gate init --project-root /tmp/review-quality --preset rust-api`.
2. Review and copy quality.toml, the referenced packs and QUALITY.md into the
   repository. Set project name to the existing flow project name, choose a stable
   quality ID, and align source roots, component IDs and full/ci/hook profiles.
   Preserve existing execution commands and security configuration.
3. Run `harness-gate config check`. It validates configuration and referenced
   policy files; it does not prove collector installation or measurement success.
4. Provision fresh signed collector requests and host-owned per-profile state and
   trusted keys as described in [collector orchestration](quality-collectors.md)
   and [trusted compilation](quality-compilation.md). Runtime files are under
   `.harness-gate/runtime/`. Even hook needs trusted state. Init does not install
   adapters, generate signing keys or build those runtime inputs automatically.
   Until they exist, verify fails closed with the missing workflow input.
5. Run `harness-gate verify --profile ci --all` after provisioning. Reuse validated
   authoritative CI evidence through the [profile retention contract](quality-profiles.md).
   Reference retained evidence is never evidence for a new application's source.

Pack series record the accepted reference toolchain/identity boundary. The host
must resolve its actual installed collector and pin matching measurement series;
changed toolchains need explicit series and baseline migration. There is no
implicit debt waiver: baseline defaults to `none`. Before enabling debt/ratchet,
configure a required [baseline provider](quality-baselines.md), compatible lineage
and the workflow baseline request. Removing quality.toml explicitly restores
flow-only behavior; retain policy and evidence for review and rollback.

This change adds no hosted collection job or duplicate collection owner. Required Quality Aggregate and hosted integration acceptance remain controller/CI owned;
The [workflow closure record](quality/gh-187/validation.md) separates retained
prerequisite hosted success from submitted-head acceptance. A green configuration
check is not a quality PASS or a claim of hosted acceptance.
