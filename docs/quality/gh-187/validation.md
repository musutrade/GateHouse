# GH-187: Configured workflow documentation and closure evidence

Historical submission record. GH-198's [final hosted reconciliation](hosted-closure.md)
supersedes the pending-at-submission status below with current-head run
34422630969, attempt 1, and successful Required Quality Aggregate job 102704478218.

Scope: OpenSpec [tasks 9.1–9.4](../../../openspec/changes/integrate-generic-quality-into-project-workflow/tasks.md)
of `integrate-generic-quality-into-project-workflow`, following merged
[GH-186 / PR #196](https://github.com/musutrade/Harness-Gate/pull/196).
This submission updates documentation and CLI help; execution, policy, schemas,
collectors, thresholds and CI topology are unchanged. The submitted commit's
required hosted checks and Required Quality Aggregate remain controller-owned
and pending. This record does not mark the whole proposal accepted.

## Product and extension acceptance

The [workflow guide](../../quality-workflow.md), both READMEs, configuration
entry points, ABOUT, architecture closure and CLI help lead with
**`init -> verify -> one project decision`**. `quality evaluate` and `adapter run`
are advanced explicit-input interfaces. Reference init generates configuration
and packs, not collectors, signing keys, signed requests or trusted runtime state.
Configuration validation alone cannot establish quality PASS.

**Ecosystem/language/framework identity is configuration/pack data. The generic
schema, compiler, orchestrator, baseline, profile, verify, report and CI layers
require no core code change for a new language name.** Extension follows
**collector + capability/policy pack + certification**, without generic-core
redesign. The existing metric contracts still govern supported semantics; an
unknown capability is rejected rather than acquiring meaning from its name.

Current Rust and Angular reference presets are bounded examples. Future
Vue + Go + PostgreSQL combinations should compose component and independent
contract/capability packs. Registry installation/version resolution and execution
pack composition are future work. PostgreSQL currently supplies an execution
service unless a quality capability is explicitly selected. Certification belongs
to an evidenced collector/toolchain/series, never to a preset or arbitrary name.
See [TypeScript certification](../typescript-certification.md),
[Rust function-risk support](../function-risk.md) and
[ADR-0047](../../adr/0047-composable-quality-preset-packs.md).

## Unknown-ecosystem end-to-end evidence

The unchanged [shared acceptance matrix](../../../tools/harness-gate/fixtures/workflow/ci-matrix.json)
contains `nebula-unregistered-2049`, alongside Rust, Angular/reference and mixed
shapes. It supplies arbitrary producer/tool/runtime/series identities and a
required `bundle.size <= 10 bytes` policy. Its passing value is 5 bytes; its
quality failure is 15 bytes. A third case makes required capability data unavailable.
These are synthetic measurements, not certification of Nebula or a real app.

The Rust test
`configured_ci_acceptance_retains_parity_reuse_and_negative_evidence`
executes all 12 shape/outcome cases through the shared configuration, compilation,
signed collector, verify, Rust evaluator, report and CI-receipt path. Each case
launches its producer once, removes the executable, then reuses authenticated
responses/artifacts with zero additional launches. Complete direct-evaluation
and verify project reports match. Forty negatives (ten per passing shape) block
digest, binding, schema, invocation, series, subject, capability, artifact, source
and missing-envelope changes. No language-specific job or dispatch is added.

The full suite also exercises the remaining generic layers:

| Test | Evidence |
| --- | --- |
| `unknown_ecosystem_pack_composes_without_catalog_or_core_changes` | Unknown pack composition and cross-plane validation; missing/conflicting bindings fail; reinitializing generic preserves explicit quality adoption. |
| `unknown_ecosystem_verify_baseline_and_direct_evaluator_are_equivalent` | Signed unknown-ecosystem collection with an available retained baseline, changed-subject selection, unified/report-file equality and direct Rust parity. |
| `trusted_baselines_preserve_lineage_and_reject_stale_unknown_ecosystem_inputs` | Git and retained providers transport arbitrary ecosystem/language/series identity, preserve supported metric lineage and reject stale inputs and unsupported metric evaluation. |
| `unknown_pack_configures_cheap_and_expensive_profiles_without_ecosystem_dispatch` | Capability metadata drives cheap/expensive/custom profile participation. |
| `ci_reuses_authoritative_responses_without_relaunching_or_changing_decisions` | Retained all/mixed producer reuse preserves decisions without duplicate measurement. |
| `quality_validates_and_round_trips_without_enabling_legacy_flow` | Optional quality config preserves flow-only compatibility. |

Sources: [preset tests](../../../tools/harness-gate/src/preset/tests.rs),
[configuration tests](../../../tools/harness-gate/src/config/quality/tests.rs),
[collector/verify tests](../../../tools/harness-gate/src/config/quality/collectors/tests.rs),
[shared CI acceptance](../../../tools/harness-gate/src/config/quality/collectors/ci_acceptance.rs).
The Python architecture guard enforces the unchanged generic-core boundary.
None of these synthetic fixtures broadens Rust or Angular certification.

## Hosted prerequisite and cost

Retained [run 34419625673, attempt 1](https://github.com/musutrade/Harness-Gate/actions/runs/34419625673)
completed successfully for GH-186 PR head
`024f775904eb6bd50639092d3779d9e2f8dad035`. Collection and receipt identity pin
the tested PR merge revision `ffe3bc0e5f7dea4f864ceab653b26debb78d235d`,
base `9fcc319d976aed1aad4311355cece60df798ff96`, Linux/X64 and run/attempt.
The PR and tested workflow blobs both equal
`502f79d8b6e9a3af43927569ca99b888c324a63c`.
Every event-required job succeeded, including Linux/macOS/Windows tests,
production quality coverage and
[Required Quality Aggregate](https://github.com/musutrade/Harness-Gate/actions/runs/34419625673/job/102695650692).
Existing push-only jobs are skipped by their event rules; this change adds no
conditional platform policy. This is prerequisite evidence, not GH-187 CI success.

The downloaded `workflow-acceptance-34419625673-1` artifact is retained as a
[compressed receipt](https://github.com/musutrade/Harness-Gate/blob/d9a932b23dcfea709175285b594ca200450a9971/docs/quality/gh-187/hosted-prerequisite-receipt.json.gz) and
[validated summary](https://github.com/musutrade/Harness-Gate/blob/d9a932b23dcfea709175285b594ca200450a9971/docs/quality/gh-187/hosted-prerequisite-summary.json), with its receipt hash and
original GitHub artifact digest.
The validator rechecks exact identity, all 12 cases, embedded file hashes,
direct/verify equality, retained inventories, launch counts and all 40 negatives.
[Raw run/jobs and collection-log identity](https://github.com/musutrade/Harness-Gate/blob/d9a932b23dcfea709175285b594ca200450a9971/docs/quality/gh-187/hosted-prerequisite-input.json),
[artifact metadata](https://github.com/musutrade/Harness-Gate/blob/d9a932b23dcfea709175285b594ca200450a9971/docs/quality/gh-187/hosted-prerequisite-artifacts.json) and
[normalized timing](https://github.com/musutrade/Harness-Gate/blob/d9a932b23dcfea709175285b594ca200450a9971/docs/quality/gh-187/hosted-prerequisite-timing.json) preserve the audit trail.

Using the existing `ci_timing.py` model and the accepted
[after cohort](../ci-topology/after-state.md):

| Measure | Accepted after-cohort median | GH-186 completed run |
| --- | --- | --- |
| Run creation to required aggregate completion | 878.5 s | 979 s |
| Summed runner wall minutes | 39.3 | 39.5833 |
| Linux / macOS / Windows runner wall minutes | 26.9 / 5.1 / 7.4 | 29.6333 / 2.3667 / 7.5833 |
| Production quality collection | 836.5 s | 934 s |
| Linux Test job wall time | 224 s | 215 s |
| Required aggregate wall time | 7 s | 8 s |

The acceptance test itself took 20.101651443 seconds within the existing Linux
Test job. Receipt validation rounded to 0 seconds at GitHub timestamp precision;
upload took 2 seconds (not a claim of zero validation cost). Production quality
collection remained the last required child. One later run with a larger test
corpus cannot attribute the overall latency increase to this integration or
establish a speedup. No production coverage/risk/CRAP recollection, new job,
compiled-cache/build-once optimization or threshold relaxation is introduced.
Launch ledgers supply the direct no-duplication evidence; these wall minutes are
unweighted runner occupancy, not billing. No new hosted run was dispatched or polled.

## Local checks and retained evidence

The focused shared acceptance test passed before the CLI help edits. The final
full Rust run passes all 379 tests; its [compressed receipt](https://github.com/musutrade/Harness-Gate/blob/d9a932b23dcfea709175285b594ca200450a9971/docs/quality/gh-187/local-receipt.json.gz)
and [validated summary](https://github.com/musutrade/Harness-Gate/blob/d9a932b23dcfea709175285b594ca200450a9971/docs/quality/gh-187/local-summary.json) retain the 12 cases and 40 negatives.
Cargo commands use `CARGO_TARGET_DIR=$PWD/target` because the default
`/home/gem/cargo-target` is outside this workspace's write permission. The final
nextest run retains a fresh receipt with empty hosted identity fields, clearly
distinguished from the downloaded hosted receipt.

The [command manifest](https://github.com/musutrade/Harness-Gate/blob/d9a932b23dcfea709175285b594ca200450a9971/docs/quality/gh-187/validation.json) records exact commands, exit codes,
environment, durations and SHA-256 hashes; [retained logs](https://github.com/musutrade/Harness-Gate/blob/d9a932b23dcfea709175285b594ca200450a9971/docs/quality/gh-187/validation-logs.tar.gz)
include complete output, help captures, check runners and the docs/contract reports.

| Final local check | Result |
| --- | --- |
| `cargo nextest run --manifest-path tools/harness-gate/Cargo.toml --locked` | 379 passed, 0 skipped |
| `cargo fmt --manifest-path tools/harness-gate/Cargo.toml -- --check` | Passed |
| `cargo clippy --manifest-path tools/harness-gate/Cargo.toml --all-targets -- -D warnings` | Passed |
| `python3 -m unittest discover -s tools/quality/tests -v` | 346 passed |
| `python3 tools/quality/docs_consistency.py --output target/quality/docs-consistency.json` | Passed; links, examples, migration and both schemas synchronized |
| `openspec validate integrate-generic-quality-into-project-workflow --strict` | Passed |
| Release Python tests / offline installer tests | 23 passed / passed |
| Python byte compilation / locked release build | Passed / passed |
| CLI contracts, structured and Linux textual snapshot | Passed; only intended root help text changed |
| `cargo audit --deny warnings --file tools/harness-gate/Cargo.lock --db target/quality/gh-187/advisory-db` with workspace `CARGO_HOME` | Passed; 207 dependencies, 1,243 advisories |
| Local / prerequisite-hosted `ci_acceptance.py` receipt validation | Both passed; 12 cases, 40 negatives each |
| `git diff --check` | Passed |

The initial Python suite recorded one docs consistency error while closure links
were being edited; the focused five-test recheck and final 346-test suite pass.
The Linux snapshot initially detected the expected help-text change; only the
`help.stdout` difference was reviewed and accepted before a normal rerun passed.
Both initial results remain in the archive. Initial audit failed to lock
`/home/gem/.cargo/advisory-db..lock`; moving only its database still warned about
`/home/gem/.cargo/.package-cache`. The final run moves both database and Cargo home
inside this workspace and passes without that warning or policy relaxation.

To inspect or revalidate the retained matrix, decompress each receipt into a
separate directory as `receipt.json`, then run `python3 tools/quality/ci_acceptance.py
--directory <directory>`. Use the hosted identity environment recorded in the
manifest for the hosted receipt; leave those fields unset for the local receipt.
The original artifact digest, compressed-file hashes, decompressed receipt hashes
and embedded per-file hashes identify different boundaries and are all retained.

Root `harness-gate config check` and `harness-gate verify --profile ci --all`
are **not applicable**, not passed: `.harness-gate/flow.toml` is absent. No generic
configuration was invented. Configured temporary fixtures exercise both paths.
Current-head native hosted jobs, production coverage and the Required Quality
Aggregate await the normal PR workflow; local success cannot substitute for them.

## Rollback and scope boundary

Flow-only compatibility and v1 migration tests retain the existing execution
path; generic reinitialization preserves adopted quality. Required quality,
execution/audit failure, unavailable baseline and invalid retained input tests
prevent a favorable fallback. A reviewed rollback can restore a previous
accepted integration/configuration while retaining reports, series and baselines.
Removing quality restores reduced-assurance flow-only operation and cannot
satisfy required quality acceptance. Required CI policy continues through the
released Rust evaluator; no Python or collector approval fallback is permitted.
The accepted [CI rollback record](../ci-topology/after-state.md#individual-rollback-decisions-task-64)
remains applicable. This documentation-only change needs no data migration.

Java/Python/third-ecosystem adapters, compat deprecation, risk-driven conditional
hosted platforms and real application dogfood stay outside this change. No
follow-ups are created before acceptance. Related design records are
[ADR-0045](../../adr/0045-quality-verification-composition.md),
[ADR-0046](../../adr/0046-capability-driven-quality-profiles.md),
[ADR-0047](../../adr/0047-composable-quality-preset-packs.md) and
[ADR-0048](../../adr/0048-configured-ci-workflow-acceptance.md).
