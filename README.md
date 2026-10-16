# GateHouse

> Development now lives in **GateHouse**. The `harness-gate` CLI and crate names remain unchanged. Historical releases through v0.4.8 retain their Harness-Gate download and signing identities. See [migration and test data](docs/gatehouse-migration.md).

[![CI](https://github.com/musutrade/GateHouse/actions/workflows/ci.yml/badge.svg)](https://github.com/musutrade/GateHouse/actions/workflows/ci.yml)
[![Crates.io](https://img.shields.io/crates/v/harness-gate.svg)](https://crates.io/crates/harness-gate)
[![Documentation](https://docs.rs/harness-gate/badge.svg)](https://docs.rs/harness-gate)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

[English](https://github.com/musutrade/GateHouse/blob/main/README.md) | [简体中文](https://github.com/musutrade/GateHouse/blob/main/README.zh-CN.md)

`Harness-Gate` is a configuration-driven project verification CLI, implemented in Rust. Its primary workflow is **`init -> verify -> one project decision`**: declare execution and quality intent, then combine security, architecture, command and evidence-based quality gates in one blocking result.

It handles changed paths, secret scanning, architecture auditing, environment validation, external command orchestration, test result counting, timeout and interrupt handling, and temporary service lifecycle. Git hooks only keep launchers, and flow decisions do not depend on Shell scripts.


Local command reuse is explicitly opt-in; see the [deterministic step cache](docs/step-cache.md) for input binding, Unix host trust, explanations and safe clearing.

## Repository and release boundaries

| Name | Responsibility |
| --- | --- |
| **GateHouse** | Active source repository for Core, configuration, schemas, orchestration and quality decisions. |
| **`harness-gate`** | Unchanged CLI and crate name; configuration remains under `.harness-gate/`. |
| **[GateProbes](https://github.com/musutrade/GateProbes)** | Independently delivered collectors and ecosystem presets. |
| **[Harness-Gate](https://github.com/musutrade/Harness-Gate)** | Historical source, issues, release assets and signing identities through Core 0.4.8. |

The source migration did not publish a new Core version. The current checkout
uses the external preset loading shipped in Core 0.4.9. Use the
version-specific quick-start path below. Ordinary source builds need no
historical evidence download; full regression and documentation checks use
[pinned optional data](test-data/README.md). See the [documentation index](docs/README.md)
and [migration acceptance](docs/gatehouse-migration-acceptance.md).

## Release status

Postgres externalization is accepted with the published 0.5.0/0.1.1 pair; see the [fixed-artifact receipt](docs/releases/postgres-service-0.1.1-acceptance.md).

**Current Core 0.5.0 is published and verified on GitHub and crates.io.** Postgres checks and migration belong to GateProbes; legacy configurations require migration. See the [upgrade guide](docs/releases/0.5.0.zh-CN.md) and [public delivery receipt](docs/releases/0.5.0-public-verification.md).

The first split release history follows.

**Core 0.4.9 is published on GitHub and crates.io.** It is the first split Core release from
GateHouse: ecosystem presets are external and the Core release inventory
contains only Core binaries and metadata. GateProbes delivers collectors
independently. See the [release guide](docs/releases/0.4.9.zh-CN.md) and
[public verification receipt](docs/releases/0.4.9-public-verification.md).

The public crate matches the original `v0.4.9` source and passed a fresh Linux
registry installation; see the [crate verification receipt](docs/releases/0.4.9-crate-public-verification.md).

The accepted [Phase 1 baseline](docs/benchmarks/phase-1/README.md) is retained;
the legacy #346 candidate was rejected after evidence review. Historical 0.4.8
releases and their plugin assets retain their original identities. The current
installer selects GateProbes native **0.1.0-rc.8** independently of Core.

## Navigation

- Quick Start: see [Installation](#installation) and [Quick Start](#installation-and-quick-start)
- New Project Integration: see [Installation and Quick Start](#installation-and-quick-start) and [Built-in Presets](#built-in-presets)
- Add Commands, Components or CI Profiles: see [Command Overview](#command-overview), the [English schema v2 configuration reference](https://github.com/musutrade/GateHouse/blob/main/docs/configuration.md), and the [JSON Schema catalog](https://github.com/musutrade/GateHouse/blob/main/schema/README.md)
- Handle Failures: see [Reports and Notifications](#reports-and-notifications) and [Common Repair Paths](#common-repair-paths)
- Extend Rust Engine: see [No Code Change Scope](#no-code-change-scope), [Rust Change Boundary](#rust-change-boundary) and [Project-owned Validation Boundary](#project-owned-validation-boundary)

## Working Model

`init` generates configuration; `verify` selects a profile and scope, runs execution
gates and configured collectors, and asks the Rust evaluator for the quality
decision. The unified report combines every required gate. See the
[quality workflow and architecture](docs/quality-workflow.md) for setup and trust.

Execution uses four types of data:

1. **scope rule**: Maps Git changed paths to components
2. **profile**: Selects different intensity steps from the same component, such as `hook`, `full`, `ci`
3. **step**: Declares a `program + args[]` external command, timeout, log, parser and service dependencies
4. **gate**: Fixed to run secret scan and audit first, only allowing external steps to execute after success

Data flow when running `verify`:

```text
Git changed files
  -> scope.rules
  -> components
  -> steps matched by component + profile
  -> secret scan
  -> architecture audit
  -> execute steps in configured order
  -> configured quality collectors + trusted baseline + Rust policy evaluation
  -> one project decision + JSON / Markdown / optional HTML/JUnit reports
  -> optional HTTP(S) Webhook notification
```

Execution mechanics live in `flow.toml`; quality intent lives in `quality.toml`.
Ecosystem, language and framework identity is configuration/pack data. A new
language name requires no change to the generic schema, compiler, orchestrator,
baseline, profile, verify, report or CI layers. Extension follows **collector +
capability/policy pack + certification**; support claims require evidence for the
actual collector/series. New metric semantics require separate contract review.

## Installation

### Install from crates.io

With Rust 1.88 or newer:

```bash
cargo install harness-gate --version 0.5.0 --locked
```

[Publication and installation verification](docs/releases/0.5.0-public-verification.md)
records the registry checksum, original tag source and fresh Linux smoke test.

### Install from GitHub Release (Pre-built Binaries)

Download the binary for your platform from an immutable [GitHub Release
tag](https://github.com/musutrade/GateHouse/releases/tag/v0.5.0):

- **Linux (x86_64)**: `harness-gate-linux-amd64`
- **macOS (Intel)**: `harness-gate-macos-amd64`
- **macOS (Apple Silicon)**: `harness-gate-macos-arm64`
- **Windows (x86_64)**: `harness-gate-windows-amd64.exe`

The checked installer verifies the checksum manifest and Sigstore certificate
before changing the installation directory. Download the current installer from
the pinned source revision, then select the immutable Core release explicitly:

```bash
curl --fail --show-error --location --proto '=https' --tlsv1.2 \
  -o /tmp/harness-gate-install.sh \
  https://raw.githubusercontent.com/musutrade/GateHouse/v0.5.0/install.sh
bash /tmp/harness-gate-install.sh --version v0.5.0
harness-gate --version
```

The script installs to `~/.local/bin` by default. Set `--install-dir` to an
existing private directory when a different location is required. It never
uses the mutable `releases/latest` API or a `raw/main` installation command.
The installer provisions a pinned `cosign` verifier when it is missing;
source installation additionally requires `git` and Rust `cargo`.

### Optional Rust collection

Core installs only Core. GateProbes owns native plugin installation, upgrade and
rollback through an explicit version and checksum-verified fixed installer.
The current source installer rejects legacy plugin flags with migration guidance.
See the [standalone guide](docs/quality/standalone-installation.md) for executable
commands and the two repositories' trust boundaries. Missing plugin dependencies
are diagnosed; Rust/LLVM, Python and Cosign are provided explicitly by the user.
Installing a plugin does not adopt a measurement series or baseline.
Historical tagged installers retain their original pins and identities.

Release assets include `SHA256SUMS`, a CycloneDX SBOM, and Sigstore signatures/certificates. For
a manual integrity check, download all listed assets, `SHA256SUMS`, and the matching
`.sig`/`.crt` files, then run:

```bash
sha256sum --check SHA256SUMS
cosign verify-blob --signature harness-gate-linux-amd64.sig \
  --certificate harness-gate-linux-amd64.crt \
  --certificate-oidc-issuer https://token.actions.githubusercontent.com \
  --certificate-identity 'https://github.com/musutrade/GateHouse/.github/workflows/release.yml@refs/tags/v0.5.0' \
  harness-gate-linux-amd64
```

Verify `harness-gate.sbom.cdx.json` with its corresponding signature as well;
the SBOM records the source commit, Cargo.lock digest, and Rust toolchain used
for the build. The release workflow first writes one explicit
`release-inventory.json`; checksum, Sigstore signature/certificate, GitHub
provenance, verification, and upload subjects are all derived from that same
inventory, including the SBOM. Missing, extra, unsigned, or unattested assets
block release creation. Published release assets are immutable: a replacement
requires a new version tag and a new attestation.

### Install from Source

```bash
git clone https://github.com/musutrade/GateHouse.git
cd GateHouse
cargo install --locked --path tools/harness-gate
```

## Development Commands

Install Rust 1.88+ (see the crate's `rust-version`), Git, Python 3 and
`cargo-nextest` for the commands below. Prepare pinned regression inputs before
full tests; the download is separate from building or installing Core.

```bash
python3 tools/maintenance/test_data.py --list
python3 tools/maintenance/test_data.py
```

The repository uses `cargo-nextest` for fast, isolated test execution:

```bash
cargo nextest run --manifest-path tools/harness-gate/Cargo.toml --locked
cargo clippy --manifest-path tools/harness-gate/Cargo.toml --locked --all-targets --all-features -- -D warnings
cargo fmt --manifest-path tools/harness-gate/Cargo.toml -- --check
```

`release` keeps the default panic unwinding behavior for diagnostics. Use
`release-small` for the distributed binary when size matters:

```bash
cargo build --manifest-path tools/harness-gate/Cargo.toml --release
cargo build --manifest-path tools/harness-gate/Cargo.toml --profile release-small
```

Worker execution is wrapped at the scheduler boundary so a panic becomes a
failed, published result. A poisoned service or lease lock also fails closed;
it is never treated as an available resource. The `release-small` profile uses
`panic = "abort"`, so callers must not rely on unwinding from that binary.
The stable machine failure registry is documented in
[docs/failure-codes.md](docs/failure-codes.md).

## Installation and Quick Start

### Core 0.5.0 and current source builds

Core 0.5.0 and the current source build use an external ecosystem catalog.
Historical Core 0.4.8 embeds its presets; with that version omit `--presets-dir`.

After installing from source, select a reviewed GateProbes checkout's `presets/`
directory. Core embeds only `generic`; see [external preset setup](docs/quality-presets.md).
Verify installation and explore presets:

```bash
harness-gate --version
harness-gate presets --presets-dir /path/to/GateProbes/presets
```

Initialize in an existing Git project (run `git init` first for a new directory):

```bash
harness-gate --project-root /path/to/new-project init --preset rust-api --presets-dir /path/to/GateProbes/presets
```

### Validate configuration and provision quality inputs

```bash
harness-gate --project-root /path/to/new-project config check
harness-gate --project-root /path/to/new-project doctor
# Review generated QUALITY.md; provision trusted state, signed requests and keys.
harness-gate --project-root /path/to/new-project verify --profile ci --all
```

Recommended integration order:

1. Select the preset closest to the tech stack
2. Review execution in `.harness-gate/flow.toml` and quality, policies and profiles in `.harness-gate/quality.toml`
3. Add project-specific architecture rules in `.harness-gate/audit.toml`
4. Add business or vendor-specific credential rules in `.harness-gate/secrets.toml`
5. Run `config check` to resolve reference and schema errors first
6. Run `doctor` to fill in local tools, dependencies, images or environment variables
7. Provision the host inputs described in generated `QUALITY.md`, then run `verify --profile ci --all` for one project decision
8. Use the same command in CI, and optionally install thin hooks that only call `harness-gate hook`

Reference presets do not install collectors, sign requests or create trusted runtime
state. Until those inputs exist, quality verification fails closed. Follow the
[quality quick start](docs/quality-workflow.md#quick-start) for the setup boundary.

`init` creates the target directory, but `scope` and `verify` require the target directory to be a Git worktree. Refuses to overwrite when the project already has configuration; only use `--force` when confirming the target content is replaceable.

<a id="built-in-presets"></a>
## Project Presets

For Core 0.5.0, generate the Postgres catalog with the installed GateProbes plugin before selecting it. The frozen legacy catalog requires migration; see [the preparation steps](docs/quality-presets.md).

Core embeds only `generic`. The three ecosystem presets below are owned by
GateProbes and require a reviewed checkout selected through `--presets-dir PATH`
or `HARNESS_GATE_PRESETS_DIR`. See [external preset setup](docs/quality-presets.md).
These loader options require a Core build containing the preset migration;
released 0.4.8 still embeds the earlier presets.

| Preset                    | Purpose                     | Initial Steps                           |
| ------------------------- | --------------------------- | --------------------------------------- |
| `generic`                 | Any Git project             | working tree and staged whitespace check |
| `rust-api`                | Single Rust crate           | fmt, Clippy, check, test                |
| `angular-only`            | Angular/npm                 | lint, format check, test, build         |
| `angular-rust-postgres`   | Angular + Rust + PostgreSQL | Dual-end check, test, build and temporary DB |

`init` writes `.harness-gate/flow.toml`, `.harness-gate/audit.toml`, `.harness-gate/secrets.toml` and `.harness-gate/.gitignore` with same-directory temporary files and atomic rename, will not leave half-written files, and will not overwrite existing configuration unless explicitly passing `--force`. `config migrate` uses the same atomic write strategy for target configuration, and generates Secret Scan v2 default rules when missing. Newly created audit v2 files preset lexical configuration for Rust, TypeScript, JavaScript, SQL, TOML and YAML, and can directly append the first rule.

The three ecosystem reference presets also compose `.harness-gate/quality.toml`,
policy/capability packs and `QUALITY.md`, with full/CI quality participation.
Rust binds accepted CRAP policy; Angular CRAP remains unsupported; PostgreSQL
remains an execution service. Provision trusted collector inputs before verify.
`generic` and flow-only migration remain opt-in. See [quality presets and explicit
migration](docs/quality-presets.md) for setup and the future composable pack model.

Presets are starting points, not runtime branches. After initialization is complete, all behavior is determined by the project's internal TOML: you can rename components, customize profiles, switch to MySQL/Redis, adjust directories or replace any steps without retaining preset original names. Audit configuration is project-local: the selected project's `flow.toml` determines a literal repository-relative `paths.audit_config`; use explicit `--project-root` and `--config` options when selecting a different project or workflow file.

## Command Overview

| Command                                                      | Purpose                                   |
| ------------------------------------------------------------ | ----------------------------------------- |
| `harness-gate presets`                                       | List generic and the explicitly selected external catalog             |
| `harness-gate init --preset <name>`                          | Generate execution configuration and reference quality packs |
| `harness-gate init --interactive`                           | Select an available preset by number and confirm initialization |
| `harness-gate doctor [--strict] [--json]`                    | Execute environment checks declared in config |
| `harness-gate cleanup [--dry-run] [--json]`                 | Inspect or reclaim stale resource leases  |
| `harness-gate scope [--staged\|--base REF\|--all] [--json]`  | List changes and selected components      |
| `harness-gate secrets [--staged] [--json]`                   | Scan tracked file contents for high-confidence credential patterns; report filenames only |
| `harness-gate audit [--json]`                                | Execute regex architecture rules and generate audit report |
| `harness-gate verify`                                        | Execute default profile based on workspace changes |
| `harness-gate verify --plan [--json]` | [Explain selection, dependencies and inputs without executing](docs/verification-plan.md) |
| `harness-gate plugins --lock PATH --json` | [Verify locked offline preset plugins with host-selected keys](docs/plugin-lock.md) |
| `harness-gate verify --profile ci --all`                     | Produce one project decision for all configured components |
| `harness-gate hook`                                          | Execute hook profile on a private staged snapshot |
| `harness-gate step <id>`                                     | Run a single step after passing secrets/audit |
| `harness-gate config check`                                  | Validate schema, references, paths, environment overrides, and resource safety |
| `harness-gate config import --execution-only`                | Import compatible Arc-Flow v2 declarations with a parity/UX report; [runtime limits](docs/dogfood/arc-admin/import/README.md) remain explicit |
| `harness-gate config check --format json`                    | Emit stable field-path diagnostics for editors and CI          |
| `harness-gate config print --resolved`                       | Output final effective configuration      |
| `harness-gate config migrate`                                | Convert schema v1 to v2                   |
| `harness-gate schema export`                                 | Generate JSON Schema for flow.toml         |
| `harness-gate adapter run --request <PATH> --trusted-key <PATH>` | Advanced: execute a signed measurement request; no policy approval |
| `harness-gate quality evaluate --help`                        | Advanced: explicit trusted inputs to the same Rust evaluator |
| `harness-gate parse-logs`                                    | Extract JSON Lines ERROR trace context    |

All commands support global `--project-root <PATH>` and `--config <PATH>`. Commands return 0 on success; configuration errors, gate failures, step failures, timeouts or interrupts return non-zero, suitable for direct use in CI.

`cleanup --dry-run` is safe to run before a retry or on a shared CI host. It
only lists lease records carrying the `harness-gate` owner marker and writes
the structured observation to `<reports>/cleanup.json`. Lease schema `2`
records the canonical project, logical resource, invocation, complete runtime
labels, container name, and immutable runtime object ID. Without `--dry-run`,
expiry only makes a lease eligible for reclaim: the runtime object is inspected
again and its filename, labels, project, resource, invocation, and immutable ID
must all match before removal. Unknown, malformed, renamed, active, or
ambiguous leases are retained, and a failed stop remains a blocking cleanup
failure for a later retry.

Interactive `verify` renders a progress bar and colored pass/warning/failure markers. Output stays plain when redirected or in CI. Use `--color auto` (default), `--color always`, or `--color never`; `NO_COLOR` disables automatic color.

## Typical Workflow

### Daily Development

```bash
# Confirm scope before coding
harness-gate scope

# Only verify hit components after coding
harness-gate verify

# Directedly re-run a single step; still goes through secrets and audit first
harness-gate step frontend.tests
```

When there are no changes in the workspace, the default scope does not select components, and `verify` only runs fixed gates. `--all` must be explicitly passed when confirmation of the entire repository is needed.

### Before Commit

```bash
git add <explicit file list>
harness-gate scope --staged
harness-gate hook
git commit -m "..."
```

A project can configure pre-commit to execute `harness-gate hook`. The command materializes the complete Git index into a private staged snapshot before loading configuration, selecting scope, running built-in gates, or executing ordinary steps. It therefore does not mix staged and unstaged file contents. Choose suitable pre-commit checks in the project hook profile; it does not replace complete testing.

Configured steps use `input = "snapshot"` by default, so `{root}`, path aliases, and arguments resolve against the invocation input root. A step that genuinely needs the original checkout or direct Git metadata may opt into `input = "repository"`; this is an explicit compatibility capability and does not change scope, secret-scan, or architecture-audit input. Invocation reports record the input mode, source identity, execution root, and configuration digest.

### Before PR or Release

```bash
harness-gate verify --all
```

This command ignores changed paths, selects all components in the configuration, and executes the default `full` profile.

### Compare Against a Baseline

```bash
harness-gate scope --base origin/main
harness-gate verify --base origin/main
```

`--base REF` uses committed changes from `REF...HEAD`, does not include uncommitted workspace content, suitable for CI or PR branch verification.

### Manually Specify Scope

```bash
harness-gate verify --components backend
harness-gate verify --components backend,frontend --profile full
```

Explicit components override automatic scope, and cannot be used with `--staged`, `--base`, `--all` at the same time. Unknown components or profiles fail immediately.

## Error Codes

Operational failures are printed as `ERROR [E####]: message`. The code is stable
enough for CI logs and support requests; the message keeps the file path or Git
command context needed to resolve the issue.

| Code | Category |
| ---- | -------- |
| `E1000` | General command, project, or configuration failure |
| `E1101`-`E1103` | Audit configuration, execution, or log parsing |
| `E1201`-`E1202` | Secret-scan configuration or execution |
| `E1301`-`E1304` | Git scope, scope configuration, unmatched paths, or reports |
| `E1401`-`E1404` | Verification selection, cancellation, execution, or reports |

### Common Repair Paths

For a new repository, use the shortest repeatable setup:

```bash
harness-gate init --preset generic
harness-gate config check
harness-gate doctor
harness-gate verify --all
```

When `config check` fails, the human output includes the stable error code,
field path, a repair hint, and a minimal schema v2 `flow.toml` shape. Apply the
hint (or regenerate with `harness-gate init --preset generic`), then run
`harness-gate config check` again. Use `--format json` when an editor or CI
needs machine-readable diagnostics.

For daily work, inspect `harness-gate scope`, then run `harness-gate verify`.
Before a commit, stage the intended files and run `harness-gate scope --staged`
followed by `harness-gate hook`. Before a PR or release, run
`harness-gate verify --all`; upload the reports directory when a failure needs
review.

## Evidence architecture and adapter support

`verify` compiles validated configuration and trusted runtime state, orchestrates
collectors and baselines, and invokes the released Rust evaluator for policy,
ratchets, contracts and project reports. Its combined status blocks on any
required execution or quality failure. Collectors measure; Rust decides. The
Required Quality Aggregate checks required CI children and retains release
responsibility. Failed `quality evaluate` commands print up to five blockers from
the completed report, with bounded, redacted policy/subject/state and available
reasons. Optional failing gates are not reported as blockers unless the aggregate
lists them. The full report remains authoritative; its JSON and exit status do not
change. Engineering-policy semantics are unchanged.

`quality evaluate` and `adapter run` remain advanced replay,
debugging and integration interfaces. Python generic modules are
[frozen reference code](docs/quality/python-retention.md).

Rust and TypeScript/Angular are current reference presets with bounded
[certification evidence](docs/quality-workflow.md#capabilities-and-certification),
not an ecosystem allowlist. Future Vue + Go + PostgreSQL combinations compose
packs; registry installation and new ecosystem certification are future work.
See [architecture and extension](docs/quality-workflow.md#architecture-and-extension)
and [workflow closure evidence](docs/quality/gh-187/validation.md), including the
synthetic unknown-ecosystem end-to-end fixture.

## Features

### Multi-component Workflow Management
- Intelligent component selection based on Git changes
- Configurable profiles (hook, full, ci)
- Parallel step execution
- Complete timeout and interrupt handling

### Security Gates
- **Secret Scan**: High-confidence credential detection
- **Architecture Audit**: Regex architecture rules
- Enforced before all external steps
- Scans reject individual files larger than 16 MiB to keep memory use bounded

### Environment Check (Doctor)
- Tool version validation
- Environment variable checking
- Docker service availability
- Git configuration validation

### Test Service Management
- Docker or Podman temporary containers
- Random port allocation
- Health checks
- Automatic cleanup
- Selected services are prewarmed while the mandatory secret and architecture gates run

### Flexible Configuration
- Schema v2 TOML configuration
- Path aliases and placeholders
- Environment variable overrides
- Custom test parsers
- Optional HTML/JUnit reports and HTTP(S) Webhook notifications

## Git Hook Integration

In a configured consumer project, save the following as executable
`.githooks/pre-commit`, then run `git config core.hooksPath .githooks`.
The project defines which steps its hook profile runs; Harness-Gate does not
exclude database tests or builds globally:

```sh
#!/bin/sh
set -eu
root="$(git rev-parse --show-toplevel)"
cd "$root"
exec harness-gate hook
```

The hook is only responsible for locating the root directory and launching the binary, all selection, gates and steps remain in Rust and TOML.

## CI Integration

CI is recommended to execute the full profile rather than relying on workspace diff on the runner:

```bash
harness-gate config check
harness-gate doctor --strict
harness-gate verify --all
```

To reuse external test services, inject the service configuration's `external_env` into the job; otherwise, pre-pull configured images and allow runner access to Docker daemon. Caching Cargo, npm and build directories only affects performance, should not skip `verify --all`.

Whether successful or failed, it is recommended to upload the `[paths].reports` directory as an artifact. This preserves audit line numbers, step timings and complete logs.

## Reports and Notifications

Every verification writes canonical evidence below `reports/invocations/<invocation_id>/`, including
`invocation.json`, `scope.json`, step logs, and the machine/human reports. The compatible
`test_result.json` and `test_result.md` files are mirrored at the report root for existing consumers. A
repository-contained HTML template can additionally produce `test_result.html`, and `report_templates.junit`
can write JUnit XML under the invocation directory. HTML templates use Tera and support `include`, `extends`,
and `block`; the full serialized report is available as `report` alongside the legacy direct fields. Report
output paths are containment-checked, including existing symlinks, and published files use temporary-file plus
atomic-rename semantics.

`test_result.json` follows the versioned [machine-result schema](https://github.com/musutrade/GateHouse/blob/main/schema/machine-result.schema.json). It keeps
the legacy `passed` field while exposing stable `status` values, per-step `attempts`, structured `failures`,
invocation-relative `artifacts`, and `evidence_complete`; consumers should use `status` and these structured
fields instead of parsing Markdown or log text.

Runner results also record parser mode/version and completeness. JUnit, TRX, and
JSON parsers are preferred over regex compatibility mode; malformed, zero, and
partial result files map to `RESULT_PARSE_FAILURE`, `RESULT_ZERO`, and
`RESULT_PARTIAL`. JUnit accepts one `testsuite` or `testsuites` root and TRX one
`TestRun` root (namespace prefixes are supported); missing/multiple roots and
non-whitespace content outside the root fail closed. Bounded retries expose `retry_count` and `flaky`, while shard
results expose a merge identity and reject missing or duplicate test identities.
An approved expiring waiver is machine-distinct as `WAIVED` and includes its
approval and compensating-control evidence.

For JSON parsers, configure `count_path` when the producer exposes a numeric
summary or a nested result array:

```toml
[parsers.json-results]
kind = "json"
count_path = "summary.total"
minimum = 1
```

Without `count_path`, only a bare root array or one recognized result array
(`testcases`, `testCases`, `test_results`, `testResults`, or `results`) is
discovered, including through object wrappers. Arbitrary numbers such as
`duration_ms`, unrelated arrays, non-array recognized fields, and ambiguous
multiple candidates are rejected with `RESULT_PARSE_FAILURE`; explicit invalid
paths never fall back to discovery.

Each invocation also publishes `artifact-registry.json` and `manifest.json`, described by the
[artifact registry schema](https://github.com/musutrade/GateHouse/blob/main/schema/artifact-registry.schema.json)
and [artifact manifest schema](https://github.com/musutrade/GateHouse/blob/main/schema/artifact-manifest.schema.json).
The registry and manifest record every invocation-local evidence file (excluding control files) with its relative
path, kind, byte size, and SHA-256 digest; step artifacts also carry invocation and step bindings. Result
declarations, registry entries, manifest entries, and publishable files on disk must match as one closed set.
Missing, undeclared, escaped, symlinked, replaced, or digest-mismatched evidence fails publication and leaves
`evidence_complete` false. The final machine result is written only after the manifest has passed validation.
Text evidence is redacted before publication: authorization and cookie headers, bearer/basic credentials, API keys,
passwords, private-key blocks, and common database connection strings are replaced with `[REDACTED]`. The default
retention policy keeps the newest 50 invocation directories and only removes older directories after the 15-minute
lease window; active or recently modified invocations are retained.

Optional `[[notifications.webhooks]]` entries send the serialized report to HTTP(S) endpoints after all report
files are written. Each URL must have an exact `allowed_hosts` entry; credentials, wildcards, local-only resolved
addresses, and redirects are rejected at configuration or connection time. `on_failure` defaults to `true`,
`on_success` defaults to `false`; a non-2xx response, connection error, or policy denial returns `E1404` while
preserving the generated reports. Entries are sent in configuration order; the first failure stops notification
delivery to later endpoints. Destination evidence is limited to scheme and normalized host.

Service cleanup is part of verification success. If a started service cannot be stopped, verification remains
failed and the cleanup error is included in `test_result.json`/`test_result.md` so a leaked container is not hidden.

For a DevRail migration, replay a serial request and compare normalized results:

```bash
harness-gate compat run --input request.json --output result.json --old-result frozen.json
harness-gate compat compare --old frozen.json --new result.json --output comparison.json
harness-gate compat canary --state migration-canary.json --slice team-a
harness-gate compat rollback --state migration-canary.json
```

These commands retain raw result digests and never delete invocation evidence.
The P2 signed adapter boundary is specified separately in
[ADR-0033](docs/adr/0033-signed-out-of-process-adapter-protocol.md). A host can
execute one signed adapter request without loading code into the CLI process:

```bash
harness-gate adapter run \
  --request adapter-request.json \
  --trusted-key adapter-key.json \
  --allow-resource test-database
```

The v2 host verifies an Ed25519 signature over the canonical complete request
(adapter identity, invocation/step, arguments, input, environment,
capabilities, timeout, configuration digest, artifact root, nonce, and validity
window) and the executable SHA-256 digest before starting the adapter. It
clears inherited environment variables, applies the declared capability
allowlist, enforces bounded stdout/stderr and artifact budgets, attempts bounded
process-tree cleanup on timeout or cancellation, and rejects malformed results
or artifacts outside the invocation root. Adapter failures are reported as
`ADAPTER_PROTOCOL_FAILURE`. Nonces are single-use. `verify` and `quality collect` share durable replay
storage; use global `--replay-state-dir PATH` for persistent host-controlled
storage outside read-only snapshots and untrusted mounts. `adapter run` can
share that override. See [replay scope and lifecycle](docs/quality-collectors.md#durable-replay-protection).

The request is a single JSON document; it is not read from `flow.toml` and does
not enable adapters in built-in steps. Repeat `--trusted-key` for every trusted
Ed25519 key and pass only the capabilities that the invocation is allowed to
use (`--allow-network`, `--allow-resource`, or `--allow-environment`). Keep the
request, executable, and artifact root inside the project or another explicitly
managed deployment directory, and review the signer before execution. The
capability allowlist is a protocol-level declaration check, not an
operating-system network, filesystem, resource, or process sandbox; process
cleanup is best effort and is not proof of complete descendant containment.
The host retains at most 16 MiB per output stream by default and returns a
structured truncation failure when a limit is exceeded.

`parse-logs` reads JSON Lines in bounded streaming passes. It keeps at most 20 records before the first matching
error and 30 records in the output; when no trace ID is available it emits only the last 30 raw lines. This avoids
loading an unbounded application log into memory.

## No Code Change Scope

The following changes only require edits to `.harness-gate/flow.toml` or the
project audit TOML:

- add or rename components, profiles, and external steps;
- connect Go, Java, Python, Node, or other CLI tools;
- adjust monorepo scope rules and path aliases;
- add Doctor checks expressible by the existing check kinds;
- add regex result parsers, Docker/environment services, or audit rules;
- provide CI-specific timeout, image, or directory overrides.

## Rust Change Boundary

Source changes are required for a new behavior category, such as a non-Docker
service lifecycle provider, a report format that cannot be parsed by regex, a
new Doctor protocol, a new credential algorithm, or a different process
cancellation policy. These changes require tests, preset validation, and a
versioned compatibility review.

## Project-owned validation boundary

Application- and repository-specific validation belongs to the project being
verified. Harness-Gate orchestrates, ingests and decides at generic boundaries;
it does not become the implementation home for every API, E2E, integration,
smoke, migration, load or framework-specific test system. Three extension layers
cover project-owned validation:

- **Command hooks / execution gates** run project-owned commands. Harness-Gate
  owns scope/profile selection, dependencies, services, environment, timeout,
  retry, requiredness and blocking composition; the project owns test code,
  fixtures, domain assertions and tool semantics.
- **Structured result adapters** ingest reusable machine formats such as JUnit,
  SARIF or declared JSON contracts to improve diagnostics and reporting.
  Parsing a result never transfers requiredness, threshold, ratchet or release
  authority to the tool or parser.
- **Quality collector plugins** measure normalized facts for generic policy
  evaluation. Collectors remain measurement-only; the released Rust core retains
  requiredness, thresholds, baselines, aggregation and final decisions.

A new application validation tool must not force a generic-core code change when
it can run through the command-hook contract. Native support is justified only
for a reusable protocol, result format, service primitive, ecosystem/capability
pack, certification boundary or generic policy semantic. When migrating a mature
project, its existing required validation is the assurance baseline until parity
and fail-closed replacement behavior are evidenced. See
[ADR-0049](docs/adr/0049-project-owned-validation-extension-boundary.md).

## License

This project is licensed under the MIT License - see the [LICENSE](https://github.com/musutrade/GateHouse/blob/main/LICENSE) file for details.

## Contributing

Contributions are welcome! Please read [CONTRIBUTING.md](https://github.com/musutrade/GateHouse/blob/main/CONTRIBUTING.md) for details on our code of conduct and the process for submitting pull requests.

## Acknowledgments

Thanks to all contributors of the arc-admin project, Harness-Gate evolved from that excellent foundation.

## Links

- **Documentation**: [docs/](https://github.com/musutrade/GateHouse/tree/main/docs)
- **Issue Tracker**: https://github.com/musutrade/GateHouse/issues
- **Original Project**: https://github.com/musutrade/arc-admin

### Quality gate development

Local quality-tool tests and the exact CI collection command are documented in
[tools/quality/README.md](tools/quality/README.md). Coverage, function risk and
isolated critical-path evidence feed `Required Quality Aggregate` on PRs and
pushes. The critical-path collector now builds the instrumented binaries once
per collection and runs up to two isolated tests concurrently by default;
`--jobs 1` selects serial execution and `--jobs N` accepts 1 to 8 workers. See
the [collection benchmark](docs/benchmarks/critical-path-collection/README.md).
Generated baselines remain review-only candidates; adoption policy is
[ADR-0039](docs/adr/0039-required-risk-and-traceability-gates.md).

Full regression data is fetched explicitly and cached by digest; ordinary source builds need no evidence download. See [test data](test-data/README.md).

### Interactive initialization

Run `harness-gate --project-root /path/to/project init --interactive` in a terminal.
Choose a numbered preset and project name, inspect the target and replacement warning, then confirm
with `y`. Empty preset selection, `q` at preset selection, EOF or a declined confirmation changes
no project files. Existing configuration is preserved unless `--force` is explicitly given
and confirmed. Piped input is rejected; automation should use `init --preset NAME`.
The two selection modes cannot be combined.

The menu includes built-in `generic` and only an explicitly selected external
catalog (`--presets-dir`, `HARNESS_GATE_PRESETS_DIR`, or locked `--plugin NAME`).
Locked plugins retain their existing verification and explicit selection rules.
Project names default to the target directory. Override that default with
`init --project-name NAME`, including alongside `--preset NAME`. Identifiers use
the existing configuration rule: nonempty lowercase ASCII letters, digits, dots,
underscores and hyphens. Explicit names are checked against the flow identifier rule before creating the target.
In interactive mode, an empty project-name line accepts the displayed default;
EOF cancels. `--project-name NAME` supplies an editable interactive default, and
`q` is a valid project name. The name prompt accepts fewer than 4096 input bytes
including the newline; longer names can be supplied explicitly. The confirmation
shows the final name, which is shared by flow and composed quality configuration.
Quality presets also retain their existing project-ID rule: a generated quality
identifier must begin with a lowercase letter; incompatible names fail without
publishing configuration (an empty target directory may remain).
This interface uses numbered line input, without arrow navigation or project-type
detection. It does not install plugins, start services or run verify.
Generated security configuration, preset validation and engineering-policy
semantics are unchanged. Review the generated files and run `config check` next.

After successful initialization, the displayed directory is escaped metadata,
not a shell command. From that project directory, run `harness-gate config check`
to validate the generated configuration, then `harness-gate doctor` to execute
the declared dependency checks. `harness-gate verify --plan --all` previews the
configured default profile without executing gates or collectors. After reviewing
configuration and prerequisites, run `harness-gate verify --all` yourself.
Initialization only prints these suggestions; it does not run checks, install
hooks, download plugins or claim verification passed. Quality opt-in and signed
workflow prerequisites remain unchanged, as do engineering-policy semantics.
