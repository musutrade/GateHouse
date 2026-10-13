# GH-166: Cargo caches and retained artifact boundaries

GH-169 [hosted acceptance and rollback](after-state.md) supersedes the compiled
cache enablement described below: every caller now leaves target caching disabled
after repeated misses. Source caches, resolved targets and immutable evidence
validation remain enabled. The table records the GH-166 experiment.


This implements `optimize-ci-execution-topology` tasks 3.1–3.3 after GH-165.
Engineering Policy, CRAP semantics, required assurance, measurement series and
Rust authority are unchanged. ADR-0039 and ADR-0040 remain the governing records.
Required Quality Aggregate acceptance on the submitted commit is pending hosted
CI, owned by the controller. No hosted speedup is claimed here.

## Confirmed before-state

CI cached `tools/harness-gate/target` alongside Cargo sources and `~/.cargo/bin`
under keys that omitted the compiler and architecture. The Linux Build upload
used the same assumed path and ignored missing files. `contracts.py` selected an
existing debug executable without asking Cargo to establish freshness.

The initial read-only command
`cargo metadata --manifest-path tools/harness-gate/Cargo.toml --locked --no-deps --format-version 1`
resolved `target_directory` to `/home/gem/cargo-target` in this environment.
That confirms the hardcoded path can differ from Cargo's actual output. No
build was run there; validation explicitly selects a target in this workspace.

## Cargo state conventions in CI

The `cargo-state` composite action sets an absolute
`CARGO_TARGET_DIR=$GITHUB_WORKSPACE/target/ci/<RUNNER_OS>/<RUNNER_ARCH>/<class>`.
It requires Cargo metadata to resolve that exact path before restoring a cache.
The resolved Cargo home supplies source-cache paths, including when `CARGO_HOME`
is redirected. Tool executables are managed separately by the GH-165 installer.

| Jobs / class | Compilation semantics | Compiled cache |
| --- | --- | --- |
| Linux and native macOS/Windows Test / `test` | Native test; Linux also generates schema in dev | Yes, isolated by OS and profile identity |
| Clippy / `clippy` | dev/test, all features | Yes |
| Linux and native push Build / `release` | Native release, default features | Yes |
| Linux and native push CLI contracts / `contracts` | Native dev, default features | Yes; Cargo build always runs before executing contracts |
| Documentation Consistency / `docs` | dev, existing preset/configuration checks | Yes |
| Code Coverage / `tarpaulin` | LLVM instrumented | No |
| Quality Coverage / `quality` | Collector owns fresh `target/quality/candidate/build` and isolated snapshot/critical-path targets | No |
| Quality Performance Baseline / `performance` | release-small and cold/warm nextest samples | No |
| Quality Script Tests / `quality-scripts` | Tests retain their isolated fixture target overrides | No |
| Format, Security Audit, Release Governance, Generic Shadow, Aggregate | No product compilation target cache needed | None |

The quality collector's explicit fresh target overrides remain authoritative for
instrumentation; the action's default target is not a compilation cache for that
job. Its existing command/raw evidence records retain those measurement paths.
This change does not alter the separate release publishing workflow or its
release authority.

Downloaded registry index/archive and git database caches use
`cargo-sources-v1-<OS>-<arch>-<lock SHA-256>`. They contain neither installed tools
nor compiled targets. Target cache keys use
`cargo-target-v1-<identity SHA-256>-<commit>`, with restore fallback restricted to
that exact identity. Identity covers OS, architecture, resolved directory, job
class, profile/features, `rustc -vV`, Cargo version, lockfile, tracked Cargo and CI
configuration/quality tooling, Cargo home configuration, and build flags.
There is no fallback across compiler, OS, class or instrumentation identities.

These caches are disposable acceleration state within GitHub's cache scope, not
authoritative artifacts. Every required build/test/check command still runs;
cache hits never skip a required outcome. Contract checks now always run the
locked debug build and discover its target with Cargo metadata. A stale binary
or failed build cannot stand in for a successful required contract execution.
The informational Linux Build upload uses the selected release path and fails
if the binary is missing. It has no downstream consumer or release authority.

Each configured job retains `cargo-state-<job>-<OS>-<arch>-<run>-<attempt>` with
the resolved path and complete cache identity. These small records explain
cache misses and establish which mutable state a job could restore.

## Immutable quality artifact transport

The existing quality-coverage producer remains the sole owner of its measurement
series. After successful collection, `ci_artifact.py seal` adds `ci-artifact.json`
to its retained artifact. It binds repository, workflow, producer, checkout SHA,
run, attempt, OS/architecture, configuration hash and effective Rust/Cargo/
nextest/llvm-cov identity, plus the exact retained file inventory and SHA-256s.
The candidate report is itself hashed. Mutable `build` and `snapshots` trees
remain excluded from upload; hidden evidence files are included in the inventory
and upload so the two agree.

The manifest digest travels separately as a producer job output. The downstream
consumer downloads only this run/attempt's named artifact, requires that digest,
then verifies the manifest, expected identity and exact file inventory before
invoking `rust_reference.py`. There is no recollection or stale-artifact fallback.
The existing native candidate/stage/hash and projection policy checks still run
after transport validation. Generic Shadow remains advisory, with no new claim
of quality authority.

Missing producer output, missing manifest/payload, substituted manifest/tool
identity, changed/extra files, mismatched source/run/attempt/configuration,
symlinks and mutable build directories fail closed. Failed collection still
uploads available diagnostic evidence through the existing `always()` step;
without a successful seal it cannot be consumed as a verified artifact.
Successful consumer validation is retained as `artifact-validation.json` in the
generic-shadow evidence upload. SHA-256 provides integrity against the expected
producer output; it is not a signature or authorization for untrusted workflow
code, and does not turn PR evidence into release authority.

## Linux reuse evaluation (task 3.3)

| Candidate consumer | Evaluation |
| --- | --- |
| Linux CLI contracts | Retain debug build. Build emits release; changing optimization profile would change the existing contract without equivalence evidence. |
| Documentation checks | Retain current Cargo execution. A binary injection interface and equivalent validation belong to task 4.1. |
| Performance/size | Retain native release-small and cold/warm measurements. Neither release nor instrumented output matches that series. |
| Coverage/risk/critical paths | Retain fresh instrumented producer isolation. No release/performance reuse. |
| Generic projection | Preserve existing retained-evidence reuse, now with an independently anchored transport manifest. No recollection. |
| macOS/Windows tests and push consumers | Retain required native execution; Linux artifacts cannot substitute for it. |

No new Linux binary-sharing dependency is introduced. This avoids an unmeasured
serialization of the PR path and keeps task 4.3's hosted comparison separate.
Hosted cache transfer overhead, hit rates and critical-path/cost deltas remain
tasks 6.1–6.4, not locally established performance results.

## Validation and diagnosis

The retained [validation record](https://github.com/musutrade/Harness-Gate/blob/d9a932b23dcfea709175285b594ca200450a9971/docs/quality/ci-topology/cargo-artifacts-validation.json) gives exact
commands, outcomes and limitations. Regression tests cover path mismatch, cache
identity dimensions, stale executable/build failure and the artifact rejection
cases above. A real Cargo metadata smoke test confirms the configured directory.

For cache problems, inspect the job's Cargo state artifact and metadata output;
delete an obsolete cache or bump its namespace, preserving all required commands.
For artifact failures, compare the producer seal step/job output, downloaded
manifest and current run identity. Repair the producer/transport; do not bypass
verification or substitute earlier evidence.

`harness-gate config check` and `harness-gate verify --profile ci --all` are not
applicable: this source checkout has no `.harness-gate/flow.toml` declaring `ci`.
No synthetic project configuration was created. Hosted Required Quality
Aggregate and native runner execution remain pending the submitted PR's CI.

Implementation references: [Cargo metadata](https://doc.rust-lang.org/cargo/commands/cargo-metadata.html),
[GitHub cache scope and keys](https://github.com/actions/cache#cache-scopes), and
[immutable artifact uploads](https://github.com/actions/upload-artifact#not-uploading-to-the-same-artifact).
