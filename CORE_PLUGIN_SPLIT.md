# Core / Plugin Split Evaluation (GateHouse / GateProbes)

Status: evaluation v2 — **dependency cut before repository move**
Scope: evaluate the core/plugin boundary and the order of work. This is **not** a
migration checklist. No code change, no Issue is created by this document.

Baseline reviewed: `main` @ `b854ed9`.

Legend: 🟦 GateHouse (core) · 🟩 GateProbes (plugins) · 🟨 needs decision

## 0. Decision and principle

- Split into **two repositories**: core **GateHouse** and collector plugins
  **GateProbes**. Two repos are enough.
- **Cut dependencies first, then move repositories.** Moving first would only
  turn single-repo coupling into cross-repo coupling.
- Keep the existing CLI and package name: repository names change to
  GateHouse / GateProbes, while the `harness-gate` command, crate and legacy
  install entry stay compatible for now. A full rename pulls in install,
  configuration, docs and release; the payoff does not justify binding it to the
  architecture migration.
- When moving repos, re-check **signing identities** (repository / workflow
  identity in Sigstore provenance and certificate identity), not only download
  URLs.

## 1. Boundary definition

- **Core owns**: protocol validation, execution budgets, result decision,
  thresholds and baseline comparison.
- **Plugins own**: language/tool-specific measurement, returning standard
  results.
- A plugin must **not** decide gate pass/fail and must **not** approve
  permission on its own.
- Rust, TypeScript, and API-contract collection implementations and ecosystem
  templates go to GateProbes; core keeps only generic templates and a plain file
  loader.
- Do **not** build a plugin marketplace, dynamic discovery, or a resident
  management service at this stage.

## 2. Corrections to v1

### 2.1 Fixtures are not all plugin-owned

Generic decision fixtures, invalid-evidence cases, and timeout/replay tests must
**stay with core** (they exercise the decision engine, not a collector). Only
collector-specific fixtures move to GateProbes.

### 2.2 Python/Rust authority is already decided

Python is already marked as a **frozen reference**; Rust is the decision body.
Do **not** re-run an authority migration. `quality-core/*` is the single
decision authority; Python equivalents stay as reference, not as a competing
source of truth.

### 2.3 Schemas are not merged by filename

Do not merge schemas just because basenames repeat. Determine a **unique source**
for each of three roles:

1. **Host contract** (request/decision wire format) — core.
2. **Generic evidence** — core.
3. **Plugin raw output** — plugin.

A release package may carry a generated copy, but the two sides must not edit
their own versions independently. These distinctions are confirmable in the
existing core differential tests and the Python reference implementation.

### 2.4 Some `docs/` files are build- and CI-critical

`docs/` is not documentation-only and MUST stay with core:

- `tools/harness-gate/tests/import_test.rs` compile-time `include_str!`s
  `docs/dogfood/arc-admin/{import/flow.toml,import/flow.import.json,sources/.arc-flow/flow.toml.txt}`.
- `tools/quality/ci_quality.py` archives `docs/dogfood` from **both** the base and
  head commits (`SNAPSHOT_PATHS`), so every measured commit must contain it.
- `tools/quality/docs_consistency.py` guards normative anchors in
  `docs/engineering-policy.md`, `docs/configuration*.md` and `schema/README.md`.
- quality tests reference `docs/dogfood/arc-admin/**` and `docs/benchmarks/phase-1/**`.

Removing `docs/` from history stops the core build and every quality gate.
When reducing the repository, keep the build/CI-critical subset with core.

## 3. CI ownership

- Do not split a third platform out of the core CI.
- The core repo's CI **consumes pinned plugin release artifacts**; it does not
  rebuild the latest plugin source each run.
- The plugin repo tests, packages and publishes on its own.
- Cross-repo keeps only a small compatibility test; widen validation only when
  the protocol changes.
- **Blocker to cut**: the current CI tooling still builds the Rust measurer
  itself. If this dependency is not removed, moving the repo will not actually
  reduce cost.

## 4. Postgres specialization

- Postgres URL rules, test-database naming and health checks are a **dedicated
  recipe**; move them out of the generic core.
- Keep generic process execution and resource reclamation in the host execution
  layer.
- Isolate the existing implementation first. Do **not** design a
  general-purpose arbitrary-service plugin framework just for the split.

## 5. Boundary table (corrected)

### A. Clear ownership

| Path | Owner | Notes |
| --- | --- | --- |
| `tools/harness-gate/src/**` (cli/config/scope/verify/audit/secrets/doctor/service/process/report) | 🟦 | Generic orchestration, gates, reports |
| `quality-core/**`: `model`, `evidence`, `policy`, `ratchet`, `replay`, `json` | 🟦 | Single decision authority |
| `presets/`: `generic.flow.toml`, `default.secrets.toml`, `empty.audit.toml`, `QUALITY.md`, `quality-base.json` | 🟦 | Generic templates/policy |
| `schema/`: `flow`, `machine-result`, `artifact-registry`, `artifact-manifest`, `adapter-request` | 🟦 | Host contracts |
| General decision fixtures, invalid-evidence, timeout/replay tests (subset of `tools/quality/fixtures/**` and `tools/quality/tests/**`) | 🟦 | Exercise the decision engine |
| `hooks/**`, `.github/actions/**`, `.github/ISSUE_TEMPLATE/**`, `PULL_REQUEST_TEMPLATE.md`, `codecov.yml`, `LICENSE` | 🟦 | Repo infrastructure |
| `tools/quality/rust-*` collection (`rust-measure/`, `rust-source-risk/`, `rust-native-plugin/`, `rust-native-driver/`, `rust-collector-runtime/`, `rust_native*.py`, `rust_collector_*.py`, `rust_reference.py`, `rust_equivalence.py`, `build_rust_collector.py`) | 🟩 | Rust collectors |
| `tools/quality/typescript-risk/`, `typescript_*.py` | 🟩 | TypeScript collectors |
| `tools/quality/http-json-contract/`, `collect_typescript_contracts.py` | 🟩 | HTTP / TS contract collectors |
| Collector-specific `tools/quality/fixtures/**` | 🟩 | Collector fixtures |
| `tools/quality/schema/`: `rust-collector-*.json`, `quality-evidence`, `project-config` | 🟩 | Plugin raw-output schemas |
| `.github/workflows/`: `native-collector-release.yml` (retired 2026-10-12, archived at `retired/workflows/`), `rust-collector-release.yml` (paused), `rust-measure-native.yml` (compatibility) | 🟩 | Plugin release/measurement pipelines; GateProbes publishes the native chain |
| `.github/workflows/rust-generic-shadow.yml` | 🟦 | Core reference-equivalence validation (D11) |
| `tools/release/`: `collector_*.py`, `install_collector.py`, `friendly_collector_install.py`, `download_collector_release.py`, `package_independent_collectors.py`, `production_collector_release.py`, `rust-collector-release.production.yml` | 🟩 | Plugin packaging/signing/distribution |

### B. Needs decision / to isolate

| Path / item | Issue | Direction |
| --- | --- | --- |
| `presets/*.packs.json`, `presets/packs/{rust,typescript,api-contract}.json` + `catalog.rs` `include_str!` | Ecosystem presets/packs compiled into core | Ecosystem templates → 🟩; core keeps generic templates + plain loader |
| `src/config/quality/collectors/ci_acceptance.rs` + `tests/dogfood_acceptance.rs` | Core source `include_str!`s a `quality/fixtures` file | Move the fixture into core 🟦 (it is a decision-engine fixture) |
| `src/service/postgres.rs` + `ExternalValuePolicy::IsolatedPostgres` | One database's rules in the generic config model | Isolate as a dedicated recipe 🟩; no generic service framework yet |
| `tools/quality/` Python reference (`policy_engine.py`, `project_model.py`, `project_report.py`, `risk.py`, `function_risk.py`, `cross_component.py`, `quality_common.py`, `policy_ratchet.py`) | Python shadow of core semantics | Already frozen reference; keep as reference, no re-migration |
| `tools/quality/` self-CI tooling (`ci_quality.py`, `ci_*.py`, `coverage.py`, `critical_paths*.py`, `baseline_delivery.py`, `complexity_analyzer.py`, `benchmarks.py`, `measurement_contract.py`, `contracts.py`, `docs_consistency.py`, `quality_evidence.py`, `harness_evidence.py`, `production_*`, `snapshots/`) | This repo's own quality gates, coupled to collectors | Keep as core CI, but cut the "builds the Rust measurer" dependency; consume pinned plugin artifacts instead |
| `quality-replay/` | Non-authoritative migration comparator | Keep in core 🟦 for now |
| `schema/quality.schema.json` + `quality-core/schema/*` + `tools/quality/schema/*` (6 duplicate basenames) | Three coexisting authorities | Assign a unique source per role (host contract / generic evidence / plugin output); release may carry generated copies only |
| `tools/quality/schema/collector-protocol.schema.json` | Core protocol schema in the plugin dir | Move up into core 🟦 |
| `tools/release/`: `release_policy.py`, `release_inventory.py`, `generate-sbom.py`, `production_installer.py`, `build_user_installer.py`, `publish_user_installer.py` | Core release vs plugin release share one tree | Split by target 🟨 |
| `.github/workflows/`: `ci.yml`, `release.yml`, `quality-baseline-refresh.yml`, `frontend-advisory.yml`, `arc-admin-cost-trial.yml` | Single-repo CI split per target | 🟨 |
| `install.sh` | Core installer embeds Rust collector version/workflow/cache logic | Keep core install entry compatible; split plugin install segment 🟨; re-check signing identity |
| `docs/**`, `openspec/**` | Docs/changes cover both sides | Split per repo 🟨 |
| `.claude/`, `.harness-gate/`, `WORKFLOW.lifecycle.md*`, `WORKFLOW.md` | Repo process artifacts | keep / migrate / drop 🟨 |

## 6. Recommended development order

1. **In the current repo**, tidy protocol, schema and test ownership. Do not
   change algorithms.
2. **Prove one genuinely independent plugin chain first**, using
   **TypeScript-risk**.
3. **Core self-CI switches to pinned plugin release artifacts**, then migrate
   GateProbes' install and release flow.
4. **Finally remove compatibility paths**; handle the complex Rust release chain
   and Postgres generalization separately.

## 7. Acceptance criteria

- Core can build and test **without checking out plugin source**.
- A plugin upgrade does **not** require re-releasing or re-compiling core.
- Both sides can release and roll back independently.
- Incompatible protocol versions produce an **explicit error**.
- CI time and artifact disk usage do **not** increase because of the split.

Plugins are already subprocess-based: choosing a new plugin version on the next
run is sufficient. Do **not** build hot-plugging for the split and do **not**
require a host restart.

## 8. Scope of this round

Evaluation only. No code change and no Issue creation in this round.
