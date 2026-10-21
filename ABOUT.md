# About GateHouse

Core 0.8.0 is an installed-runtime host release candidate. Its Linux read-only host does not
replace the service engine or authorize mutations. See the
[release scope](docs/releases/0.8.0.zh-CN.md) and
[protected host contract](docs/runtime-probe-installed.md).

GateHouse is the active development repository for **`harness-gate`**, a
configuration-driven project verification CLI written in Rust. Its workflow is
**`init -> verify -> one project decision`**: declare execution and quality intent,
run the configured checks, and combine their results into one blocking decision.
The CLI, crate name and `.harness-gate/` configuration directory remain unchanged.

## What it does

- Maps Git changes to components and selects checks by profile (`hook`, `full`, `ci`).
- Runs secret scanning and architecture audits before configured external steps.
- Orchestrates project-owned build, lint, test and integration commands with
  timeouts, structured result parsing and temporary service lifecycle management.
- Invokes configured collectors and evaluates their evidence against quality
  policies, trusted baselines and non-regression rules in the Rust core.
- Publishes JSON and Markdown results, with optional HTML/JUnit reports and
  webhook notifications.

Execution belongs in `flow.toml`; quality intent belongs in `quality.toml`.
Collectors measure facts; Core owns requiredness, thresholds and the final
quality decision. Missing or invalid required evidence blocks success.
See the [workflow and architecture](docs/quality-workflow.md).

## Repository responsibilities

| Project | Responsibility |
| --- | --- |
| [GateHouse](https://github.com/musutrade/GateHouse) | Core source, CLI, schemas, orchestration, policy evaluation and current issues/PRs. |
| [GateProbes](https://github.com/musutrade/GateProbes) | External collectors, ecosystem presets and independently signed plugin releases. |
| [Harness-Gate](https://github.com/musutrade/Harness-Gate) | Pre-migration history, issues and immutable Core releases through 0.4.8. |

New languages and frameworks integrate through project-owned commands or
collector + capability/policy packs. Support and certification depend on the
actual collector and measurement series; a preset name alone grants no quality
capability. Application-specific tests stay with the application.

## Version and installation

Current Core **0.5.0** is published on GitHub and crates.io with [verified installation](docs/releases/0.5.0-public-verification.md). Postgres configurations require the explicit GateProbes migration described in the [release guide](docs/releases/0.5.0.zh-CN.md). Earlier delivery history follows.

Core **0.4.9** is published on GitHub as the first split Core release from
GateHouse. See [release status](docs/release-status.md) and
[public verification](docs/releases/0.4.9-public-verification.md) for registry and
runtime acceptance. Core embeds only `generic`, loads reviewed GateProbes
presets through `--presets-dir` or `HARNESS_GATE_PRESETS_DIR`, and publishes a
Core-only inventory. The accepted performance baseline and quality/debt
semantics are retained.

The migration did not publish a new Core binary or re-sign historical assets.
The current repository installer selects GateProbes native RC8; the installer
frozen in Core 0.4.8 retains its RC7 pin. Use the
[version-specific quick start](README.md#installation-and-quick-start),
[standalone plugin installation guide](docs/quality/standalone-installation.md)
and [verified delivery record](docs/release-status.md).

Core supports Linux, macOS and Windows under the MIT license. Ordinary source
builds need no historical evidence download. Full regression and documentation
checks restore [pinned optional data](test-data/README.md), with digest-verified
caching and an explicit offline mode.

## Start here

- [English README](README.md) / [中文 README](README.zh-CN.md): installation and usage.
- [Documentation index](docs/README.md): configuration, integration and diagnosis.
- [Contributing](CONTRIBUTING.md): build, test and documentation checks.
- [Engineering policy](docs/engineering-policy.md): authority and required quality semantics.
- [Migration acceptance](docs/gatehouse-migration-acceptance.md): source continuity and validation.
- [Issues](https://github.com/musutrade/GateHouse/issues): bugs and feature requests.
- [Crate API documentation](https://docs.rs/harness-gate): published Rust API.
