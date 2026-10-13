# Design: Optimize CI Execution Topology

## Policy inheritance

This design inherits `docs/engineering-policy.md` without modification.

- CRAP semantics unchanged.
- Required assurance unchanged.
- Fail-closed semantics unchanged.
- Rust release/generic decision authority unchanged.
- Current PR-required macOS and Windows full tests remain required.

The design changes **how equivalent checks are prepared and executed**, not what evidence or outcomes are required.

## Current topology and observed pressure

The current `CI` workflow fans out independent jobs for Linux tests, macOS/Windows tests, security audit, format, Clippy, release build, quality coverage/risk/critical paths, CLI contracts, docs consistency, release governance, and quality-script tests. Pushes add cross-platform builds/contracts, coverage upload, and performance baselines. `Required Quality Aggregate` waits on the event-applicable dependency set.

Recent hosted PR evidence shows the full Windows test job at roughly 7.5 minutes and macOS at roughly 5 minutes in a representative successful run. Several Linux jobs are individually fast but independently repeat checkout/toolchain/tool installation or compilation. These figures are observations, not permanent budgets.

Task 1 evidence is retained in [the GH-164 topology baseline](../../../docs/quality/ci-topology/baseline.md).
The freeze found that native tests already execute on PRs but the aggregate's
`PUSH_ONLY` constant still included them. Task 1.3 corrects that stale
classification and tests the stated PR requirement; execution scheduling is
unchanged. The record distinguishes this observed pre-change gap from the
assurance contract and retains three comparable hosted runs before optimization.

## Design principles

### D1. Assurance topology is frozen for this change

The current event-to-required-child mapping is the compatibility oracle. Optimization may change implementation details or job decomposition only if the resulting event has the same required semantic outcomes. A child check cannot disappear merely because another job happens to run similar commands.

If a proposed optimization would make a platform/capability conditional, advisory, sampled, or scheduled instead of currently required, stop and propose a separate normative policy delta.

### D2. Hosted timing is evidence

Local timings are useful for debugging but cannot prove hosted GitHub Actions performance. Capture run/job/step timestamps from representative successful hosted runs and retain a normalized baseline. At minimum record:

- workflow/run/attempt/head/base identity;
- event type;
- job name, runner OS, conclusion, start/end/wall time;
- setup/tool-install duration where observable;
- main test/build/collection duration;
- aggregate start/end and effective PR critical path;
- approximate sum of hosted runner wall minutes, reported separately by OS rather than pretending billing multipliers are equivalent.

The after-state uses the same normalization rules.

### D3. Tool installation is versioned infrastructure

Pinned CI tools should not be rebuilt from source on every job when a maintained prebuilt installer or a validated durable binary cache provides the same executable/version contract.

Requirements:

- versions are explicit, not floating implicitly because an install action happens to choose latest;
- job evidence can report the effective tool version;
- cache keys include tool/version/OS/architecture where applicable;
- cache miss/failure falls back to a trustworthy installation path, not to skipping the check;
- untrusted PR content cannot poison a cache that later grants authority without normal GitHub cache isolation/validation.

Tasks 2.1–2.3 use a commit-pinned prebuilt installer with checksums, explicit
nextest/llvm-cov/audit versions, locked source fallback for unsupported binaries,
and mandatory effective-version checks. See the [GH-165 tool setup record](../../../docs/quality/ci-topology/tool-setup.md)
for trust boundaries, local evidence, and pending controller-hosted acceptance.

### D4. Cargo cache and target paths are explicit

Do not assume `tools/harness-gate/target` when Cargo may resolve another target directory. CI should deliberately select its target directory or query Cargo metadata where needed. Cache configuration must match the actual directory used.

Registry/git caches and compiled target caches have different invalidation and trust characteristics. A single broad cache should not be used merely for convenience if it causes stale/mutable build-state ambiguity.

Tasks 3.1–3.3 select and metadata-check OS/architecture/job-class target paths,
separate source and compiled caches, and keep measurement targets uncached.
Task 6 disables compiled caching for every caller after repeated hosted misses;
only source download caches remain enabled in the final model.
The [GH-166 boundary record](../../../docs/quality/ci-topology/cargo-artifacts.md)
defines the cache identities and retained diagnostic evidence.

### D5. Immutable artifacts may be reused; mutable workspaces may not be shared as authority

A producer may publish an immutable artifact for another job when:

- it is bound to the exact commit/run/attempt and relevant configuration/tool identity;
- hashes/manifests are retained where the artifact participates in quality authority;
- the consumer validates expected identity before use;
- missing/mismatched artifacts fail closed;
- reuse does not erase a required independent platform execution.

Examples of suitable reuse include retained normalized quality evidence and a release binary consumed only by compatible Linux contract checks if the contract explicitly allows that binary identity. Cross-platform binaries/tests remain platform-native.

The existing retained quality transfer now seals its file inventory and identity
with a manifest whose digest is passed independently through producer job outputs.
Consumer transport validation precedes the existing candidate/policy checks.
No Linux binary sharing is adopted in task 3.3: current contracts use debug,
Build emits release, and performance uses release-small. No equivalence or hosted
critical-path evidence supports changing those consumers in this task.

### D6. Measure once when measurement semantics require one owner

Coverage/risk/CRAP/critical-path collection already has provenance-sensitive semantics. The quality-coverage producer remains the measurement owner for its series. Downstream generic projection/comparison/reporting consumes retained artifacts instead of rerunning equivalent instrumented tests.

No optimization may merge incompatible measurement series merely because both are called coverage or complexity.

Tasks 4.1–4.3 evaluated building the docs executable once, preserving every
preset, migration, schema, policy and link/wording check. Task 6 reverted that
experiment after neutral hosted timing; all eleven locked Cargo operations
remain, with within-job Cargo freshness reuse. The retained producer/consumer boundary
already satisfies the collection ownership audit; a workflow regression guards
against adding a second collector. The [GH-167 repeated-work audit](../../../docs/quality/ci-topology/repeated-work.md)
records hosted input and rejects unproven Linux job merges or binary sharing.
No scheduling dependency, required outcome or measurement series changes.

### D7. Aggregate stays cheap and stable

`Required Quality Aggregate` retains its exact check name and `always()` behavior. It should:

1. receive event-applicable child results through `needs`;
2. evaluate them fail-closed;
3. emit a small aggregate result.

It must not install heavy Rust tools, compile the product, recollect coverage, or rerun tests. Python setup may be removed later if/when the same stable aggregate contract is implemented elsewhere, but changing aggregate authority is not required by this optimization.

Tasks 5.1–5.3 retain checkout, Python setup, and the existing aggregate command
as an exact tested step allowlist. The independent event fixture covers every
required child, including the unconditional macOS/Windows test matrix, and
push-only exemptions. Malformed needs payloads and required entries fail with
controlled diagnostics; CLI tests prohibit process launches and collection.
See the [GH-168 aggregate contract record](../../../docs/quality/ci-topology/aggregate.md).

### D8. Avoid job-count optimization that increases critical path

Combining jobs can save setup/runner-minutes but can also serialize independent work. Changes must be evaluated against both:

- PR critical path (developer wait time), and
- total runner work/setup duplication.

A merge is beneficial only when hosted evidence supports the tradeoff and assurance remains equal. The design does not mandate collapsing all Linux jobs into one monolith.

## Candidate implementation areas

### Tool setup

- Replace `cargo install cargo-nextest --locked --force` in quality collection with a pinned prebuilt installer where supported.
- Replace source installation of `cargo-llvm-cov` with an explicitly pinned prebuilt installation where supported.
- Replace repeated forced `cargo-audit` source compilation with pinned prebuilt installation or a validated binary cache; preserve `cargo audit --deny warnings` semantics.
- Keep version reporting in evidence/logs.

### Rust build/cache

- Standardize an explicit CI `CARGO_TARGET_DIR` convention per OS/job class where safe.
- Use cache keys that distinguish Rust/toolchain/lockfile/profile/OS as needed.
- Evaluate whether Linux `build` output can be consumed by compatible contract/docs checks without changing their semantic contract.
- Do not share instrumented coverage build output with uninstrumented performance or release claims.

### Documentation consistency

`docs_consistency.py` currently initializes/checks every preset by invoking Cargo repeatedly. The implementation may add a purpose-built Rust validation command or a single compiled binary invocation loop, provided it checks the same presets/migration/schema semantics and does not weaken the policy anchor checks.

### Quality evidence

Preserve the current retained quality candidate as the single provenance source for downstream generic/shadow/reference checks. If additional future consumers are added, they should download and verify that artifact instead of recollecting.

### Workflow structure

Reusable local composite actions or reusable workflows may centralize repeated Rust/Python/tool setup. They must be pinned to repository content at the tested commit and must not hide versions or required commands from evidence/debug logs.

## Performance acceptance method

Create a retained `ci-topology-baseline` record before implementation from multiple representative successful PR runs when available. Compare against multiple successful after-state runs to avoid claiming improvement from a single noisy hosted runner.

Report:

- median and range for PR critical path;
- median and range per major job;
- setup/tool-install time;
- approximate runner wall minutes by OS;
- number of repeated source tool installations;
- number of independent Rust compilation-heavy jobs;
- quality collection duration;
- artifact upload/download overhead.

A target such as reducing ordinary PR critical path toward ~5 minutes is directional, not an acceptance threshold. The hard acceptance criterion is assurance parity plus a demonstrated meaningful reduction in avoidable setup/runner work or critical path. If an optimization is neutral or worse, revert that optimization rather than weakening assurance.

## Rollout and rollback

Implement in bounded stages. Each stage must pass the existing Required Quality Aggregate before the next optimization is accepted.

Rollback is execution-only: restore the previous setup/cache/job implementation while preserving all required checks and retained evidence. Do not use rollback to remove platforms, lower thresholds, or change requiredness.

## Relationship to later workflow integration

This design deliberately does not introduce the product-level `quality.toml`/`verify` orchestration. It establishes a cleaner execution substrate and cost evidence so that later collectors and generic quality evaluation can be integrated without multiplying redundant CI jobs.

## Task 6 hosted decision (GH-169)

[Retained hosted comparison and parity review](../../../docs/quality/ci-topology/after-state.md)
shows materially less tool-install work, but overlapping PR latency and runner-cost
ranges. Retain pinned acquisition and immutable evidence validation. Disable
compiled caches after repeated misses; restore docs Cargo calls after neutral
hosted execution timing. No timing is extrapolated for the rollback commit.
Tasks 6.1–6.4 are evidenced separately from task 7 and overall proposal closure;
the controller owns the submitted SHA’s hosted aggregate result.

## Task 7 documentation and closure (GH-170)

The [current operating guide](../../../docs/quality/ci-topology/README.md)
documents the final execution model, tool pins, cache/artifact diagnostics and
separate critical-path/runner-cost interpretation. The
[closure record](../../../docs/quality/ci-topology/closure.md) retains GH-169's
final rollback commit's green required-CI evidence and GH-170 local validation.
Earlier stage-specific pending statements describe their submission time.
GH-170 submitted-SHA CI and final proposal acceptance remain controller-owned;
the proposal is not archived or declared wholly accepted before that result.
Future risk-driven conditional cross-platform execution requires a separate
normative Engineering Policy delta and was not implemented here.
