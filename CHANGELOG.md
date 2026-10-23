# Changelog

## 0.11.0 (unreleased)

- Add explicit Linux read-only `podman-capture` with fixed project host mapping and operator trust opt-in.
- Pin runtime protocol 0.4.0 with protected Podman context/installation, canonical capture wire and evidence codec; include MIT text in the protocol crate.
- Bound pipe stdout by deadline/cancellation and preserve original shared descriptor flags on normal/error return.
- Keep evidence persistence explicit; add no mutation, lease or freshness authority. Legacy Podman service operations remain in Core.
- Candidate verification uses fake sockets and pinned local binaries. Real Podman, rootless/restart, published adapter and importer acceptance remain pending.

## 0.10.0 (published)

- Add protocol 0.3.0 with explicit Docker lifecycle authority and protected installed-host dispatch.
- Retire Core Docker CLI operations; managed Docker requires lifecycle_host.
- Retain Core lease/recovery/cancellation coordination and legacy lease markers; no automatic migration.
- Add exact-ID operator cleanup and durable absence-based release. Podman extraction remains pending.
- Rootful normal and failed-health recovery have local real-engine evidence; rootless, restart and downloaded lifecycle product acceptance remain pending.

## 0.9.0 (published)

- Add the independently versioned v3 readiness host and protocol 0.2.0 dependency.
- Bind each installed-host invocation to the exact adapter, config and image.
- Replace fixed-sleep cancellation test synchronization with a test-only validated-response observer.
- Keep service lifecycle mutation in Core; separate v3 package/importer delivery remains pending.

## 0.8.0 (published 2026-10-21)

- Add the protected Linux installed-runtime host command and installation descriptor binding.
- Publish protocol 0.1.1 for the new tag package identity; retain strict registry checks.
- Require independently pinned onboarding 1.0.3 before release. Fake-socket read-only scope only.

All notable changes to Harness-Gate will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.7.1] - published

- Pin the runtime protocol dependency to a publishable registry version.
- Run registry packaging dry-run before publishing GitHub release assets.
- Require onboarding presets 1.0.2 for the exact Core 0.7.1 compatibility gate.

See [the repair release notes](docs/releases/0.7.1.zh-CN.md). The existing 0.7.0
GitHub release remains immutable; its crates.io publication failed.

## [0.7.0] - GitHub assets published; crates.io incomplete

- Add the Linux protected-file, sealed-input read-only runtime v2 host and strict
  engine binding contract. Remove and Absent remain refused; service engine
  selection and lease migration are unchanged.
- Externalize GitHub consumers/actions and operator lifecycle tooling; retire
  platform source-link rendering and transport validation from Core.
- Add conservative intent transitions and cooperative Unix durable storage,
  without a production mutation command or power-loss guarantee.

See [release boundaries and compatibility](docs/releases/0.7.0.zh-CN.md).
This entry describes a candidate, not published artifact acceptance.

## [0.6.0] - pending publication

- Release the explicit signed-plugin store, trusted-key and plugin preset CLI
  already integrated on main. The published 0.5.0 binary does not provide these
  commands; matching version metadata alone is not a compatibility check.
- Add content directory/index verification, pinned historical result reading,
  separate policy/runtime identities and external result consumption. Local
  verification completion is independent of delivery to external consumers.
- Complete Unix process-group cleanup before reaping normally exited leaders,
  and reject cancellation at final adapter/quality completion boundaries.
- Preserve default secret detection while constructing synthetic credential
  test inputs without source-scan false positives. Synchronize the adapter
  cancellation fixture on completed output rather than file creation.

See [the release notes](docs/releases/0.6.0.zh-CN.md) for compatibility and
trust limits. Existing unreleased notes below retain additional source history;
this entry does not claim a release tag or artifact has already been published.

## Unreleased — Issue #32

- Add opt-in `step-cache/v1` local reuse for declared deterministic external steps on Unix hosts, with private authenticated storage, complete input identity, output/parser revalidation, bounded retention and safe `cache-clear`.
- Add optional machine-readable cache explanations and the read-only plan reason; current Core decisions, quality evidence and legacy execution remain authoritative.

## [Unreleased]

### Added

- Add source-only `harness-gate findings` normalization, pinned diagnostic history,
  SARIF export and an opt-in GitHub Actions feedback integration. Explicit stable
  fingerprints retain identity across moves; Core policy and publication authority
  are unchanged. See [the contract](docs/findings.md).

- Optional version-1 component impact graphs select transitive consumers of
  changed shared components, with pinned graph inputs and full source-chain
  explanations in verification plans. Profiles and step prerequisites retain
  their existing behavior; invalid or stale graph data blocks selection.

### Fixed

- Bind managed-container teardown and stale-lease cleanup to the immutable
  runtime object ID verified against the lease, so a reused container name is
  not selected for deletion after inspection. Cleanup failures retain the lease
  marker and remain reported; no name-based fallback is introduced.

- Remove the unrelated ambient `rustc` probe from generic verification metadata.
  Reuse the optional checkout commit sampled at project discovery, with bounded
  Git execution, output and cancellation cleanup. Invocation metadata version 2
  retains `toolchain` as an `unknown` compatibility placeholder; actual runner
  and collector tool identities remain in their own descriptions and evidence.
- Apply `scope.unmatched = "all"` fallback only when changed paths include an
  unmatched path. Clean trees select no components and fully matched changes
  retain incremental selection; explicit `--all` and default fail-closed
  behavior are unchanged.

## [0.5.0] - 2026-10-14

### Breaking

- Move Postgres URL isolation, health checks, recipes and legacy migration to
  GateProbes. Core requires explicit, digest-bound `service_checks`; old v1 and
  v2 `external_value_policy` configurations require plugin migration. Core
  retains requiredness, lifecycle and final blocking decisions. Quality policy,
  thresholds, baseline/debt/ratchet and release authority are unchanged.

### Fixed

- Include both source and destination paths when selecting components for a Git
  rename in staged, working-tree and base scope. Renames no longer omit source
  component verification, regardless of Git rename settings.

### Changed

- Consume the fixed GateProbes Rust measurer release with repository/tag/workflow
  provenance verification, bounded extraction and explicit legacy rollback. The
  measurement binary, series, thresholds and debt/ratchet semantics are unchanged.

## [0.4.9] - 2026-10-13

### Changed

- Split Core development into GateHouse and independent collectors/presets into
  GateProbes. Ecosystem presets are loaded from a reviewed external directory;
  Core embeds only `generic`.
- Remove the four independent collector packages from the active Core release
  inventory. GateProbes publishes TypeScript risk, HTTP/JSON contract, Rust
  source-risk and Rust measurer separately. Historical packaging helpers and
  compatibility fixtures remain for their existing consumers and rollback.
- Move active Core source and future signed releases to GateHouse; keep the
  `harness-gate` CLI/crate name and historical releases through v0.4.8 intact.
- Restore digest-verified external regression inputs before release quality
  checks in lean GateHouse checkouts.
- Reuse Cargo download caches across lockfile changes, isolate regression caches
  by requested data groups, remove duplicate Python setup and bound CI jobs.
- Retain the accepted Phase 1 performance baseline after rejecting the legacy
  #346 candidate for its changed timed workload without a series transition.
- The required core CI risk stage consumes the published Rust measurer
  (`v0.4.8`, digest-verified through `tools/quality/rust-measure-release.json`)
  instead of compiling `tools/quality/rust-measure`. Analyzer identity and
  measurement semantics are unchanged.
- `install.sh --rust-only` (and `--with-rust`) download the native Rust/MIR
  collector from the GateProbes releases and verify the explicitly provisioned
  GateProbes workflow identity; the default collector version moves to
  `0.1.0-rc.8`. Core binary installs keep their Harness-Gate identity, and no
  quality threshold or measurement semantic changes.
- Retire the superseded native collector publication entry from this
  repository's active workflows (archived at
  `retired/workflows/native-collector-release.yml` with its restore commit
  recorded); GateProbes publishes the native collector. Historical releases,
  signatures and tags are retained, and compatibility reads (measurer source,
  fixtures, paused bundled-chain protections) are unchanged.

## [0.4.8] - 2026-10-11

### Added

- Publish the pinned Rust measurer as
  `harness-gate-rust-measure-0.3.1-linux-amd64.tar.gz` with a clean-extract
  identity check and SBOM, so core CI can consume a released analyzer instead of
  compiling `tools/quality/rust-measure` from source. Analyzer identity, the
  `mccabe-rust-3` rule and every gate threshold are unchanged.
- TypeScript-risk collector: reject an incompatible collector request schema or
  collector version with an explicit expected/received diagnostic instead of a
  bare assertion.

### Changed

- Assign one editable source per schema role (host contract, generic evidence,
  plugin raw output), mark the byte-identical mirrors as generated, move the
  collector protocol schema into the core host-contract catalogue, and relocate
  the decision/workflow fixtures into the core crate. Schema and fixture
  ownership are recorded and enforced; measured semantics are unchanged.

### Fixed

- Resolve the collector protocol schema from the installed runtime `app/schema`
  layout with a source-root fallback, and package it through the runtime build
  lock, so the assembled runtime no longer raises `FileNotFoundError`.
- Keep the benchmark integration probe out of the benchmark's own timed nextest
  suite, record the excluded binary, and retain a failed sample's full
  stdout/stderr under `nextest-samples` with a bounded head+tail diagnostic.
- Retry the benchmark worker's state lock on Windows access-denied contention so
  a parallel sample cannot fail on a transient lock race.
- Share durable adapter nonce protection between `verify` and `quality collect`.
  Global `--replay-state-dir` selects persistent host storage outside read-only
  snapshots. Atomic claims, protected directory handles and fail-closed path
  checks reject replays before collector execution, including after restart and
  artifact cleanup (GH-269).
- Retain in-memory adapter nonces through the complete clock-skew acceptance
  window, including equality and saturated timestamp arithmetic. Signature,
  source/configuration binding and quality policy checks are unchanged.

## [0.4.7] - 2026-09-22

### Fixed

- Include `ast/Cargo.lock` in the Rust source-risk RC6 archive. The public RC5
  archive omitted this runtime input, so measurement-series construction failed.
  Core 0.4.6, native RC7 and both Node collectors passed public consumer checks;
  their existing tags and assets are retained.
- Run the full Rust runtime tests against the extracted archive before signing,
  and both Node suites against clean installed runtime files. Missing package
  inputs or skipped Rust acceptance tests fail publication.
- Core evaluation and metric formulas are unchanged. Core 0.4.7 is the corrected
  delivery; native RC7, TypeScript RC4 and HTTP JSON contract RC5 retain their
  independent identities. Rust source-risk RC6 requires updated host pins.

## [0.4.6] - 2026-09-22

### Added

- Configure the shared collector artifact byte budget with
  `[limits] max_artifact_bytes` in quality.toml. Both `verify` and
  `quality collect` retain the 64 MiB default and reject invalid limits.
- Deliver independently versioned Rust source-risk RC5, TypeScript risk RC4
  and HTTP JSON contract RC5 candidate packages with signed checksums, SBOMs
  and provenance in the Core release. Node packages use pinned shrinkwraps;
  Rust source-risk includes a Linux x86-64 inventory executable.
- Document project-owned validation extensions (ADR-0049), source measurement
  boundaries, installation and explicit measurement-series migration.

### Fixed

- Rust source collection supports declarative business Serde metadata,
  JSON/format and tokio::select! source expressions, exact closure entry
  counters and async ownership. Retain compressed evidence without duplicating
  raw capture files; missing native mappings still fail.
- HTTP JSON contracts support closed business APIs, Angular write clients and
  functional interceptor imports. TypeScript risk supports signed project
  binding, file coverage and isolated shared-root artifacts.

### Changed

- Reuse one instrumented build per critical-path collection and overlap up to
  two isolated tests by default (`--jobs` accepts 1–8).
- Native Rust/MIR RC7 caches successful dependency-root attribution per LLVM
  export and adds signed monorepo source-prefix binding. Its independent
  four-platform release becomes the installer default. New plugins remain
  independent RC candidates; Core alone owns policy decisions and baselines.

## [0.4.5] - 2026-09-14

### Security

- Update locked rustls to 0.23.45 for RUSTSEC-2026-0285. Core 0.4.4 was blocked
  by the release audit and was never published; its tag remains immutable.

### Fixed

- Native plugin RC6 reads project CRAP policy and retains provenance. Core
  0.4.5 selects RC6 by default; Core metric and policy semantics are unchanged.

## [0.4.3] - 2026-09-14

### Changed

- Release Core 0.4.3 as an installer maintenance release, with no Core policy or
  collector protocol change. `--with-rust` and `--rust-only` install the native
  standalone executable from its own immutable signed release; `--rust-version`
  selects the plugin independently of Core (default `0.1.0-rc.5`). Rust/LLVM and
  Python remain external dependencies.
- Verify the plugin checksum and exact tag/workflow certificate before replacing
  either executable in a combined installation. Each executable uses the existing
  atomic replacement and private-directory checks.
- Retain `--rust-root` as a binary destination (`DIR/bin`) and `--cache-dir` for
  the pinned verifier cache. Legacy bundled-runtime offline archives require
  their historical installer; existing environments and baselines stay intact.


## [0.4.2] - 2026-09-13

### Fixed

- Finalize large Core-owned native quality JSON with bounded field redaction and exact output digest checks; retain external evidence limits and the invocation byte budget. Preserve all quality records and valid JSON instead of failing report publication solely because a generated report exceeds the per-text-file limit.

- Let staged partial quality profiles read host-owned state and keys while checking
  source/configuration pins against the Git index; create contained empty evidence
  directories in the private snapshot without importing working-tree artifacts.

## [0.4.1] - 2026-09-12

### Improved

- Keep Core installation small by default; add `install.sh --with-rust` and
  `--rust-only` for the optional Rust collector with automatic verifier provisioning.
- Compress and deduplicate collector transport into reusable toolchain and plugin
  layers while preserving every original signed archive byte and dual verification.
- Cache downloads by digest; support offline installation and bounded HTTP/1.1
  range retries for large downloads, including publication read-back verification.
- Publish reviewed installer deliveries through the existing protected collector
  workflow. Core continues to use the existing GitHub/crates.io release workflow.


## [0.4.0] - 2026-09-11

### Added

- Publish the accepted generic Rust quality Core, including `quality evaluate`,
  collector invocation, capability states, baseline/lineage and debt decisions.
- Include accepted generic project configuration and evidence contracts, native
  Rust measurement tooling, and independent collector installation/release tooling.

### Fixed

- Scope crates.io publication to the public `harness-gate` package; the workspace
  replay tool remains non-publishable.

### Compatibility and scope

- Adapter producers must use the accepted protocol v2 signed-request contract;
  review existing integrations before upgrading from 0.3.x.
- The Core release enables follow-up testing against an identifiable published
  executable. It does not certify GH-230 clean-host acceptance, publish an
  independent collector package, enable GH-215, or transfer Arc-Admin authority.
- Relevant Engineering Policy semantics remain unchanged. No threshold,
  capability, baseline, measurement-series or release gate is weakened.

### Security

- Versioned adapter requests now use protocol v2 complete-request signatures,
  nonce/expiry replay checks, and pre-spawn environment validation.
- Process output and adapter artifacts are bounded with truncation/deadline
  failures, and audit/log/report/SBOM error paths share credential redaction.

## [0.3.7] - 2026-09-03

### Added

- Added the R-07 adapter isolation wording inventory and Linux/macOS/Windows
  OS-sandbox decision matrix, plus a documentation consistency fixture that
  fails closed on unsupported sandbox claims.
- Added reproducible R-16 scheduler/wait scenario evidence and R-17
  configuration-validation allocation evidence under `docs/benchmarks`.

### Changed

- Synchronized the post-remediation hardening OpenSpec and ADR-0038 acceptance
  records with the v0.3.6 implementation and closed the umbrella.

### Fixed

- Stabilized the Windows lease heartbeat lifecycle test so transient marker
  replacement and real-time file scanning no longer cause flaky CI failures.

## [0.3.6] - 2026-09-02

### Added

- Added explicit webhook host and resolved-address egress policy with
  structured destination failures and credential-safe diagnostics.
- Added stable failure-code and retry-class documentation, bounded process
  output handling, and structured configuration diagnostics.
- Added English configuration/schema references and an English JSON Schema
  catalog alongside the Chinese configuration guide.

### Changed

- Added staged snapshot input contracts, dependency-aware scheduling indexes,
  bounded retry backoff, runner isolation validation, and lease ownership
  evidence for managed services.
- Added repository security, contributor, quality-script, and release metadata
  governance checks.

### Scope boundary

- This release does not claim an OS-level adapter sandbox, DevRail staging
  acceptance, shadow/canary approval, or rollback authority. Those remain
  separately governed follow-up work.

## [0.3.5] - 2026-09-01

### Fixed

- Fixed the tag-triggered release workflow so the publication directory only
  downloads platform release artifacts and excludes retained release-policy
  evidence.
- Re-ran the immutable release path under a new patch version after the
  `v0.3.4` publication stopped at exact inventory verification.

## [0.3.4] - 2026-09-01

### Added

- Added the opt-in `harness-gate adapter run` command for signed, isolated
  out-of-process adapters with Ed25519 and executable digest verification,
  capability allowlists, timeout/cancellation cleanup, and artifact confinement.
- Added release checksum, CycloneDX SBOM, Sigstore signature, and GitHub build
  provenance generation and verification to the release workflow.
- Added staged-snapshot invocation inputs, runtime ownership proof for Docker and
  Podman leases, closed artifact registry/manifest publication, and an explicit
  release asset inventory with exact-set verification.

### Changed

- Release publication now runs the locked quality, formatting, lint, dependency
  audit, schema, and offline inventory gates before credentials are available.
- Privileged release workflow actions are pinned to reviewed commit identities.
- The installer now requires an immutable version tag, verifies the release
  checksum manifest and Sigstore certificate, and atomically replaces only a
  regular destination file.

## [0.3.3] - 2026-08-30

### Fixed

- Corrected README navigation anchors and made repository documentation links
  work from GitHub, crates.io, and docs.rs.
- Added explicit docs.rs package metadata and expanded the crate-level CLI
  documentation target.
- Confirmed production and test targets have no unused imports under strict
  Clippy, and no unused dependencies under `cargo +nightly udeps`.

## [0.3.2] - 2026-08-30

### Fixed

- Hardened working-tree secret and audit scans against symlink traversal and
  non-regular files.
- Made cancellation stop built-in scans promptly and terminate complete
  process trees on Windows.
- Corrected audit rule attribution, allowlist handling, Doctor path resolution,
  parser error propagation, and service cleanup diagnostics.
- Reduced repeated scope and audit matcher compilation and synchronized CLI,
  installation, and remediation contracts.

## [0.3.1] - 2026-08-29

### Fixed

- Added a documentation-only library target so docs.rs can build the package
  without exposing the CLI's internal modules as a public runtime API.

## [0.3.0] - 2026-08-29

### Added

- `config check --format json` now emits versioned, field-path diagnostics for
  configuration parse, interpolation, validation, and resource conflicts.
- Configuration preflight rejects unordered shared services, unordered service
  injection-name collisions, and duplicate step logs before execution.
- Optional `[report_templates]` paths are validated as repository-contained,
  disjoint read-only template input; optional HTML and JUnit report output is
  now available without changing the legacy JSON/Markdown files.
- HTML report templates are rendered with Tera, including repository-contained
  `include` and `extends`/`block` composition.
- Verification plans now use dependency-aware scheduling with serial
  compatibility by default and opt-in bounded parallel execution through
  `[execution]`.
- Docker-backed services can select the Docker-compatible Podman runtime with
  `runtime = "podman"`; Docker remains the default.
- Optional `[[notifications.webhooks]]` entries deliver the serialized report
  over HTTP(S) after report writing.

- Verification steps may declare `depends_on`; invalid dependency graphs fail during configuration validation and selected steps run in stable topological order.
- Added `harness-gate config schema` to export `schema/flow.schema.json`.
- Added `${NAME}` and `${NAME:-default}` configuration environment interpolation.

## [0.2.0] - 2026-08-27

### Added
- Terminal-aware colored output and interactive verification progress bars
- Configurable `--color auto|always|never` output mode with `NO_COLOR` support
- ADR-0022 and OpenSpec documentation for terminal feedback

### Added
- Stable CLI error codes for audit, secret scan, scope, and verification failures

### Changed
- Refactored audit, secret scan, scope, and verification boundaries to use typed errors while preserving actionable error context
- Consolidated report output and shared Git snapshot/path handling in internal utilities
- Updated all Rust dependencies to latest semver-compatible versions
  - serde: 1.0 → 1.0.229
  - serde_json: 1.0 → 1.0.151
  - toml: 1.1 → 1.1.4
  - ignore: 0.4 → 0.4.33
  - regex: 1.13 → 1.13.1
  - globset: 0.4 → 0.4.20
  - tempfile: 3.8 → 3.27
- Security patches and performance improvements from dependency updates

## [0.1.0] - 2026-08-26

### Added
- Initial public release of Harness-Gate (extracted from arc-flow)
- Renamed from `arc-flow` to `harness-gate`
- Configuration directory changed from `.arc-flow/` to `.harness-gate/`
- Environment variables prefix changed from `ARC_FLOW_` to `HARNESS_GATE_`
- Standalone project structure with independent development workflow
- Complete documentation for independent usage (English + Chinese)
- MIT License
- CI/CD workflows with GitHub Actions
- Installation script for easy setup
- Badge support for CI, crates.io, and docs

### Changed
- Binary name: `arc-flow` → `harness-gate`
- Project structure: Independent repository instead of subdirectory
- Version set to 0.1.0 for initial public release

### Maintained
- All original functionality from arc-flow
- Schema v2 configuration format
- Secret scanning capabilities
- Architecture audit system
- Multi-component workflow management
- Docker service integration
- Git hook integration
