# Optimized CI execution model

This is the current operating guide for `optimize-ci-execution-topology` after
GH-169's rollbacks. Earlier stage records describe experiments; this guide and
the [closure evidence](closure.md) distinguish retained behavior from those
experiments. The [Engineering Policy](../../engineering-policy.md),
[ADR-0039](../../adr/0039-required-risk-and-traceability-gates.md) and
[ADR-0040](../../adr/0040-language-agnostic-evidence-policy.md) still govern.
CRAP <= 30, coverage thresholds, critical-path requirements, measurement
identities, debt/ratchet rules and Rust decision/release authority are unchanged.

## Execution and ownership

The [CI workflow](../../../.github/workflows/ci.yml) fans out Linux tests and
schema sync, full native macOS/Windows tests, security audit, format, Clippy,
release build, quality coverage/risk/critical paths, CLI contracts, docs
consistency, release governance and quality-script tests on every PR.
Push additionally requires native builds/contracts, Code Coverage and Quality
Performance Baseline. The [frozen event matrix](baseline.md) and
[aggregate contract](aggregate.md) specify the exact child mapping.

`Required Quality Aggregate` keeps its exact name and `always()` condition.
Its only work is checkout, Python setup and fail-closed evaluation of `needs`.
Missing, malformed, failed, cancelled or unexpectedly skipped required children
block it; only existing event-specific push-only exemptions are allowed.
It does not compile, test or collect replacement measurements.
A documentation-only change is an explicit, validated selection rather than an
unexpected skip: `classify` and the pure documentation checker must succeed,
heavy children may be skipped only when the classifier selected the reduced set,
and a heavy child that ran and failed still blocks. See the
[documentation-only routing contract](../ci-docs-routing.md).

Quality Coverage and Critical Paths owns collection for each series. Generic
Quality Shadow verifies and projects that retained evidence; it remains advisory.
Base/head snapshots and isolated critical-path targets remain distinct. No
downstream recollection, cross-platform substitution or cross-job product binary
sharing was introduced. Tests, dev contracts, release builds, Clippy and
instrumented measurements retain their own compilation semantics and parallel
scheduling. Contracts always ask locked Cargo to establish executable freshness.
Docs run all eleven locked Cargo operations, reusing Cargo's within-job target;
the build-once docs experiment was reverted without losing any checks.

## Tool versions

| Tool | Required version | Use |
| --- | --- | --- |
| cargo-nextest | 0.9.143 | Full tests, quality collection, benchmarks and baseline refresh |
| cargo-llvm-cov | 0.9.0 | Quality collection |
| cargo-audit | 0.22.2 | Security and release audits; deny-warnings preserved |
| cargo-tarpaulin | 0.37.5 | Push-only Code Coverage; LLVM engine and XML evidence retained |

The [installer](../../../.github/actions/install-ci-tool/action.yml) pins
`taiki-e/install-action` to `183e4297cca2404691e9380e1307288dced5c82a`, enables
checksums and verifies effective versions through the consuming Cargo command.
Missing platform/version binaries use the explicit locked source fallback;
download/checksum failures fail the job. There is no installed-tool cache or
cache-hit exemption from verification. See [tool setup](tool-setup.md) for
platform details and the pin-update procedure.

This is not a fully pinned environment: Rust remains `stable`, Python `3.x`,
runner images use `*-latest`, and other actions retain their existing version
tags. Tarpaulin's pin matches the source-installed version observed before
GH-291; its installation now uses the same verified prebuilt/locked-fallback
contract. Retained effective tool identities, rather than moving labels,
describe an actual measurement. Changing the remaining conventions is outside
the tool-setup optimization.

## Cache and artifact trust boundaries

The [Cargo state action](../../../.github/actions/cargo-state/action.yml) selects
`$GITHUB_WORKSPACE/target/ci/<OS>/<architecture>/<class>` and requires Cargo
metadata to agree. Collector-owned fresh instrumentation targets override that
default. Download caches contain only the resolved Cargo home's registry
index/archives and git databases, keyed by
`cargo-sources-v1-<OS>-<architecture>-<lock SHA-256>`. A same-OS/architecture
prefix fallback reuses downloads after lockfile changes; locked Cargo still
resolves and verifies the requested dependencies.
They contain no tools, compiled products or authoritative measurement evidence.
They are disposable acceleration state within GitHub cache isolation; restoring
a cache grants no quality or release authority.

**Compiled target caching is disabled for every caller.** The retained action
implementation is experimental infrastructure, not an enabled optimization.
Its identity includes compiler, Cargo, OS, architecture, lock/configuration,
flags, class and profiles; weakening those boundaries to manufacture cache hits
is not an accepted remedy. Instrumented quality, performance and ordinary
targets are never exchanged as equivalent measurements.

The quality producer seals `ci-artifact.json` with repository/workflow/producer,
checkout SHA, run/attempt, platform, configuration and effective tool identity,
plus exact file inventory and SHA-256s. The manifest digest travels independently
through a producer job output. The consumer downloads the named current
run/attempt artifact and validates that digest, identity and inventory before
existing candidate/projection policy checks. Hidden evidence is included;
mutable build/snapshot trees, symlinks and extra/missing/changed files are rejected.
Hash integrity is not producer authentication and cannot authorize untrusted PR
workflow code or release publishing. Linux Build's resolved binary upload is
informational. Failed collection can upload diagnostics via `always()`, but
unsealed diagnostics are not valid input for successful projection.

## GateHouse setup optimization (2026-10-13)

Regression data cache keys include the requested groups as well as the manifest
hash and OS. A job restoring only replay/corpus data can no longer win the
immutable cache key needed by jobs that also require quality history. Every
restore still validates pinned archive and member digests. Jobs using the
regression-data action use its Python setup instead of repeating the same setup
later in the job.

All primary CI jobs have explicit timeouts (5–45 minutes according to workload),
including the always-running aggregate. A timeout remains a failed required
check. Events, required checks, native platforms, collection commands and quality
thresholds are unchanged. Compiled targets remain uncached; Codecov retains its
independent tarpaulin series. The observed PR run 38358858096 spent about 14m26s
in quality collection; these setup changes do not establish a reduction in that
critical path. Hosted cache-hit and duration comparisons remain to be measured.

## Diagnosis

| Symptom | Evidence to inspect | Recovery preserving required assurance |
| --- | --- | --- |
| Tool installation/version failure | Resolve/install/verify steps, selected platform, checksum/fallback reason, expected and actual version | Repair the pinned installation or reviewed pin contract; rerun the required job. Never skip its gate. |
| Unexpected cache miss or target mismatch | `cargo-state-<job>-<OS>-<architecture>-<run>-<attempt>` artifact, `target/quality/cache/*.json`, Cargo metadata and cache action logs | Confirm resolved Cargo home/target and lock identity. Source-cache misses can download normally. Remove obsolete disposable cache state or revise its namespace with review; keep target caching disabled. |
| Missing or rejected quality artifact | Producer collection/seal outcome, manifest-digest output, artifact run/attempt name, `ci-artifact.json`, consumer expected identity | Repair producer or transport and rerun. Never reseal substituted content in the consumer, use a stale run, bypass verification or recollect a replacement series. |
| Projection fails after valid transport | `artifact-validation.json` in Generic Shadow evidence and retained candidate/policy reports | Investigate the existing semantic failure; transport success does not imply policy success. |
| Aggregate is red | Event, complete `needs` results and first failed required child | Fix that child's failure. Do not alter requiredness or add heavy recovery work to the aggregate. |

## Hosted comparison and remaining work

The [hosted comparison](after-state.md) retains three baseline runs and two
post-optimization runs under the same normalization, with source inputs,
job/step timestamps and log audit. These samples precede the final rollbacks;
the [rollback commit's green run](closure.md) proves required-CI acceptance of
that code but is not a new matched performance cohort.

| Measure | Before median (range) | After median (range) |
| --- | --- | --- |
| Developer critical path, seconds | 903 (846–1141) | 878.5 (875–882) |
| Total runner wall minutes | 40.7 (37.5–43.2) | 39.3 (38.3–40.3) |
| Linux runner minutes | 28.6 (28.2–33.3) | 26.9 (26.2–27.5) |
| macOS runner minutes | 4.3 (3.2–4.9) | 5.1 (4.8–5.3) |
| Windows runner minutes | 5.7 (5.7–7.5) | 7.4 (7.3–7.5) |
| Summed tool-install seconds | 423 (392–453) | 12 (12–12) |
| Quality collection seconds | 600 (560–821) | 836.5 (828–845) |

Critical path measures PR workflow run creation to aggregate completion, including scheduling
and waiting. Runner cost sums active job wall time, separated by OS, excluding
queue time and billing multipliers. Concurrent setup seconds are work, not
elapsed developer wait. Fewer jobs alone demonstrates neither improvement.

Pinned tools removed 411 median summed install seconds; audit job wall time fell
from 193 to 16.5 seconds. Quality collection remained the last required child in
every primary sample and absorbed much of the setup saving. Latency and total
runner ranges overlap; native runner work increased. These small, unmatched
cohorts establish no causal latency improvement, guaranteed percentage or billed
savings. Remaining bottlenecks are instrumented base/head/isolated collection,
native full tests and independent compilation profiles. Artifact steps were
approximately flat (10 vs 10.5 median seconds); retain their integrity checks.
Nested action times already included in setup/cleanup must not be double counted.

GH-169 disabled compiled caches after seven misses and zero hits per primary
run (41.964/32.908 summed restore/save seconds), and restored docs Cargo calls
after 43-second operation times versus baseline median 42. Future experiments
need hosted evidence for both developer wait and runner work, preserving the
same measurement identities. Revert neutral/worse or ambiguous optimizations
individually; never recover speed by weakening required assurance.

The [current #61 diagnosis](../issue-61/README.md) separates PR and push paths,
retains collection subcommand timings, and validates nextest scheduling for the
legacy coverage stage. Standalone `coverage.py` still defaults to cargo; CI
explicitly selects nextest with zero retries. Production projects those fresh
exports; risk and critical-path isolation remain unchanged. Push's observed
critical path is the macOS performance baseline, so a faster Linux collection
does not imply the same end-to-end push improvement.

## Separate policy and product changes

Any future risk-driven conditional cross-platform execution is a **separate
normative Engineering Policy delta** requiring explicit policy review and
independent compatibility/negative evidence. Path filters, sampling, scheduled
certification or advisory replacement of required native tests were not
implemented here. Full macOS and Windows tests remain PR-required; this change
provides no implicit authorization to reduce platform assurance.

Product workflow integration (`quality.toml`, verify collector orchestration,
baseline UX and preset quality integration) remains the independent
`integrate-generic-quality-into-project-workflow` follow-up.

## Supplemental measurement cadence

The [CI cadence amendment](../../engineering-policy.md#ci-cadence-amendment)
moves the independent tarpaulin/Codecov collection and three-platform performance
baseline from every main push to daily/manual `Quality Trends`. Commands, platforms,
five samples and cold measurement state are preserved. These jobs no longer hold
`Required Quality Aggregate` open; their failures remain visible in their own
workflow and must be investigated. This delays supplemental regression detection
until the next capture. No baseline is adopted automatically.

Fresh coverage, paired risk/CRAP and isolated critical paths remain required on
PRs and main pushes, as do native tests on all three platforms. The historical
push-only tables above describe the prior cadence; this section supersedes their
coverage/benchmark scheduling entries. Weekly baseline-review PR delivery remains
separate and unchanged.
