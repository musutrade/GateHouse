# Design: Core / Plugin Split (GateHouse / GateProbes)

## Context

Harness-Gate is one repository containing a generic Rust core, language-specific
collectors, plugin release pipelines, and repository-local CI tooling. The
collector protocol already isolates measurement into subprocesses, so the runtime
boundary exists; what is missing is a repository and ownership boundary that is
enforceable and that does not depend on shared source.

The split must be sequenced as **dependency cut first, repository move second**.
This design records the boundary, the decisions it depends on, and the order of
work.

## Decisions

### D1 — Boundary (responsibility), not language

- Core owns: protocol validation, execution budgets, result decision, thresholds,
  baseline comparison.
- Plugins own: language/tool-specific measurement, returning standard results.
- A plugin MUST NOT decide gate pass/fail and MUST NOT approve permission.

### D2 — Ecosystem templates

Rust, TypeScript and API-contract collection implementations and ecosystem
templates go to GateProbes. Core keeps only generic templates and a plain file
loader. Do not build a marketplace, dynamic discovery or a resident manager.

### D3 — Python/Rust authority is settled

Python generic modules are already frozen reference code; Rust is the decision
body. This change MUST NOT re-open that migration.

### D4 — Schema source of truth per role

Do not merge schemas by filename. Define one source for each role:

1. **Host contract** (request/decision wire format) — core.
2. **Generic evidence** — core.
3. **Plugin raw output** — plugin.

A release package MAY ship a generated copy, but the two sides MUST NOT edit
their own copies independently. The current duplicate basenames across
`schema/`, `quality-core/schema/` and `tools/quality/schema/` are resolved by
this role split, not by deduplication.

### D5 — Fixture ownership

Generic decision fixtures, invalid-evidence cases and timeout/replay tests stay
with core. Only collector-specific fixtures move to GateProbes. Concretely, the
fixture currently referenced by
`src/config/quality/collectors/ci_acceptance.rs` (a decision-engine fixture)
moves into core.

### D6 — Core self-CI consumes pinned plugin artifacts

Core CI MUST stop building the latest plugin source. It consumes pinned plugin
release artifacts. The plugin repository tests, packages and publishes on its
own. Cross-repository keeps only a small compatibility test; widen validation
only on a protocol change. The existing "CI builds the Rust measurer" path is the
blocker to remove.

### D7 — Postgres isolation without a service platform

Postgres URL rules, test-database naming and health checks are a dedicated
recipe and move out of the generic config model. Generic process execution and
resource reclamation stay in the host execution layer. Isolate the existing
implementation; do not design an arbitrary-service plugin framework.

### D8 — Naming compatibility

Only repository names change to GateHouse / GateProbes. The `harness-gate`
command, crate, published package and legacy install entry stay compatible. A
full rename is out of scope.

### D9 — Provenance identity

Moving the repository changes signed provenance: repository identity and workflow
identity in Sigstore certificate identity and attestation. These MUST be
re-verified in the plugin release chain, not only download URLs.

### D10 — Migration scope and provenance

GateProbes receives plugin source, dependency manifests and lockfiles, test
source, minimal test fixtures needed for independent measurement, protocol
copies, licenses, and build/install/release scripts. It does NOT receive
historical CI logs, screenshots, manual acceptance reports, build caches, old
binaries or the full Git history. The new repository starts from a clean source
snapshot that records the source repository, the full commit SHA and the path
mapping. Historical releases, signatures and records stay with the old
repository and are never regenerated or deleted.

### D11 — rust-reference equivalence stays in core

The rust-reference equivalence tests read historical candidate packages under
`docs/quality/gh-96` and `docs/quality/gh-97`. That comparison suite stays in
core; GateProbes carries only the minimal fixtures its own measurement needs.
Inputs are not deleted to make a moved test skip.

### D12 — CI topology: always-on classification with a fixed aggregate

Both repositories keep one always-started workflow: classify changes, select
subtasks, and finish in a fixed aggregate check. Selected tasks must succeed;
only tasks that are provably inapplicable may be skipped. Classification
failure, unknown changes or unresolved scope MUST fall back to the conservative
full set. `paths-ignore` is not added to a required workflow, because a
path-filtered required check can stay Pending forever.

Tiers:

1. Pure documentation: formatting, links and claim checks only, no Rust/Node
   toolchain.
2. Single-plugin change: that plugin's checks, tests, required coverage and
   pinned-Core contract acceptance.
3. Shared protocol, shared scripts, toolchain or CI-rule change: expand tests
   along the dependency edges.
4. Platform, path, process, install or packaging change: add the relevant native
   platform tests.
5. Release: only then run the full platform, packaging, clean-install, signing
   and provenance acceptance for the plugin to be released.

`ownership.json`, protocol specifications and test inputs under `docs/` are
inputs, not "ordinary documentation", and MUST NOT be classified into the pure
documentation tier. Doc-only and executable-example checks are separate:
GateHouse splits its documentation consistency check so a pure-doc change does
not invoke Cargo. Speed targets (1–2 minutes for pure docs, 5–10 minutes for a
normal plugin PR) are design goals to be measured before they are claimed.

### D13 — Migration order

1. Create GateProbes minimal source and the CI classification framework
   (classify → selected jobs → fixed aggregate).
2. Migrate one low-dependency plugin and validate the independent flow.
3. Migrate the remaining plugins under D10, keeping rust-reference equivalence
   (D11) and `rust-generic-shadow` in core.
4. Switch release and signing identity last (D9).

Core keeps consuming pinned plugin release artifacts and does not recompile on
every plugin change.

## Boundary table

### Clear ownership

| Path | Owner |
| --- | --- |
| `tools/harness-gate/src/**` | core |
| `quality-core/**` (`model`, `evidence`, `policy`, `ratchet`, `replay`, `json`) | core |
| `presets/`: generic templates (`generic.flow.toml`, `default.secrets.toml`, `empty.audit.toml`, `QUALITY.md`, `quality-base.json`) | core |
| `schema/`: host contracts (`flow`, `machine-result`, `artifact-registry`, `artifact-manifest`, `adapter-request`) | core |
| General decision / invalid-evidence / timeout / replay fixtures | core |
| `hooks/**`, `.github/actions/**`, templates, `codecov.yml`, `LICENSE` | core |
| `tools/quality/rust-*` collection | plugin |
| `tools/quality/typescript-risk/`, `typescript_*.py` | plugin |
| `tools/quality/http-json-contract/`, `collect_typescript_contracts.py` | plugin |
| Collector-specific fixtures | plugin |
| `tools/quality/schema/`: `rust-collector-*.json`, `quality-evidence`, `project-config` | plugin |
| collector release workflows (`native-collector-release`, `rust-collector-release`, `rust-measure-native`) | plugin |
| `rust-generic-shadow.yml` reference equivalence shadow | core (D11) |
| `tools/release/collector_*.py`, collector installers/packagers | plugin |

### To isolate / decide

| Item | Direction |
| --- | --- |
| `presets/*.packs.json`, `presets/packs/*` + `catalog.rs` `include_str!` | ecosystem templates to plugin; core keeps generic templates + plain loader |
| `ci_acceptance.rs` + `dogfood_acceptance` fixture | fixture into core |
| `service/postgres.rs` + `ExternalValuePolicy::IsolatedPostgres` | isolate as recipe; no generic service framework |
| Python generic reference modules | keep frozen reference; no re-migration |
| repo self-CI tooling (`ci_quality.py`, `coverage.py`, `critical_paths*.py`, …) | keep as core CI but consume pinned plugin artifacts |
| `quality-replay/` | keep in core for now |
| duplicated schemas | assign source per role (D4) |
| `collector-protocol.schema.json` | move up into core |
| `tools/release/` non-collector scripts | split core vs plugin by target |
| `install.sh` | keep core entry compatible; split plugin install segment; re-check identity |
| `docs/**`, `openspec/**` | split per repository |
| `.claude/`, `.harness-gate/`, `WORKFLOW.lifecycle.md*`, `WORKFLOW.md` | keep / migrate / drop |

## Sequencing

1. In the current repository, tidy protocol, schema and test ownership. No
   algorithm change.
2. Prove one genuinely independent plugin chain with TypeScript-risk.
3. Core self-CI switches to pinned plugin release artifacts; then migrate
   GateProbes install and release flow.
4. Remove compatibility paths last. Handle the Rust complex release chain and
   Postgres generalization separately.

## Alternatives considered

- **Move repositories first, refactor after.** Rejected: converts single-repo
  coupling into cross-repo coupling and increases detection difficulty.
- **Keep a monorepo with two top-level directories.** Rejected in favor of two
  repositories for independent build/test/release and rollback.
- **Full rebrand to `gatehouse`.** Deferred: touches install, configuration,
  docs and release; not worth binding to the architecture migration.
- **Generic arbitrary-service plugin framework for Postgres.** Rejected for now:
  isolate the existing recipe instead of designing a platform.
- **Third CI platform for self-quality tooling.** Rejected: keep it as core CI
  once it stops building collectors from source.

## Rollback plan

Because the dependency cut is additive and compatibility-preserving, rollback is
per-step: keep the `harness-gate` entry and compatibility paths until step 4; do
not delete compatibility code or old install entries until the new plugin chain
is proven green on both repositories.

## Open questions

- Which repository owns the split of `tools/release/` non-collector scripts.
- Final location of the six duplicated schema basenames once roles are fixed.
