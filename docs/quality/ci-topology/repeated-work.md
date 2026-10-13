# GH-167: Remove avoidable repeated CI work

This implements `optimize-ci-execution-topology` tasks 4.1–4.3 after GH-166.
[Engineering Policy](../../engineering-policy.md), CRAP semantics, thresholds,
measurement series, requiredness and native platform requirements are unchanged.
[ADR-0039](../../adr/0039-required-risk-and-traceability-gates.md) and
[ADR-0040](../../adr/0040-language-agnostic-evidence-policy.md) remain governing
records. Tasks 5–7 and overall proposal acceptance remain open. Submitted-commit
Required Quality Aggregate acceptance is pending controller-owned hosted CI.

## GH-169 hosted decision

The experiment below is historical. [Hosted task 6 evidence](after-state.md)
found neutral docs execution time and reverted the build-once helper to locked
Cargo calls. All eleven semantic operations and their failure tests remain.

## Documentation and preset execution

The before-state docs check passed with four presets. It invoked `cargo run`
11 times: init/check for angular-only, angular-rust-postgres, generic and rust-api;
v1 migration and migrated config check; and schema export. These calls repeated
Cargo startup and build-freshness resolution, not necessarily full compilation.

The GH-167 experiment made `docs_consistency.py` perform one locked dev binary build, then runs the
same 11 CLI operations against the executable reported by Cargo. Compiler JSON
identifies the executable for this crate; redirected targets and platform suffixes
need no hardcoded path. Cargo establishes freshness even on cache hits. A failed
build, missing or ambiguous executable cannot fall back to a stale binary.
Compiler diagnostics remain visible. The binary is reused only within this job;
there is no new artifact dependency or mutable cache authority.

All four preset init/check pairs, the v1 migration fixture, generated secrets,
migrated config validation, byte-for-byte flow schema sync, machine result/schema
catalog checks, artifact manifest/registry schema checks, policy anchors, Markdown
links, language docs and sandbox wording checks remain in place. The actual
before/after reports have identical values for all 11 semantic fields.
Regression cases also inject failure at every CLI operation, missing migration
secrets, schema drift, and failed/missing/ambiguous build output.

This removes ten compilation-capable Cargo startups per successful docs run.
Metadata version probes are unchanged. No hosted latency or cost reduction is
claimed before after-state evidence exists.

## Collection ownership audit

| Evidence / series | Collection owner and reuse decision |
| --- | --- |
| CLI legacy and expanded production coverage | `quality-coverage` collects LLVM JSON once through its legacy stage. LCOV/Cobertura exports use `--no-run`; production evaluates those same retained JSON/LCOV files via `--raw`. Keep both policy evaluations. |
| Function complexity, risk and CRAP | `quality-coverage` owns the base and head source snapshots and their instrumented nextest runs. LCOV/Cobertura exports use `llvm-cov report`; source measurement and comparison use retained counters/manifests. Base/head identities and the production coverage boundary differ; combining their measurements is rejected. |
| Critical paths | `quality-coverage` owns the mandatory row collection and retained bundle. Distinct scenarios and isolated targets remain required; they are not interchangeable coverage runs. |
| Generic/reference projection | `quality-generic-shadow` downloads the current run/attempt candidate and validates the independently supplied manifest digest, source/run/config/tool identity and full file inventory before projection. Native candidate/stage/hash and policy validation still runs. It has no compiler, collection command, reseal, or recollection fallback. |
| Push Codecov and performance | Tarpaulin and release-small/cold/warm performance have separate existing measurement contracts. Instrumented candidate artifacts cannot replace them. Event policy remains unchanged. |

GH-166 already implemented the retained transport boundary, so task 4.2 adds a
workflow ownership regression rather than another collector or transport layer.
Existing historical replay tests prohibit subprocess collection and check exact
values/identities; missing/tampered/stale evidence and incompatible series fail
closed. See [artifact boundaries](cargo-artifacts.md). Generic Shadow remains
advisory, and Required Quality Aggregate retains its existing dependency set.

## Linux compilation audit and rejected opportunities

The [audit input](https://github.com/musutrade/Harness-Gate/blob/d9a932b23dcfea709175285b594ca200450a9971/docs/quality/ci-topology/repeated-work-hosted-audit.json) derives from the three retained
[hosted baseline](baseline.md) runs, with input hash and run/source identities.
These pre-optimization samples cannot establish post-GH-166 cache hit rates or
separate compilation from tests inside a step.

| Linux job | Hosted wall seconds, median (range) | Decision |
| --- | --- | --- |
| Test | 222 (185–226) | Retain native nextest plus Linux schema export. Test harness compilation does not establish Clippy analysis or release build equivalence. |
| Build | 153 (140–154) | Retain release/default-feature build. Its uploaded executable does not satisfy the current dev contracts or release-small performance contract. |
| Clippy | 47 (42–48) | Retain all-targets/all-features analysis; neither a dev executable nor nextest output represents this semantic result. |
| Quality CLI Contracts | 59 (56–62) | Retain fresh dev build plus contract scenarios. Sharing a dev executable with docs is potentially compatible, but requires validated transport and a new scheduling dependency with no hosted after-state evidence of benefit. |
| Documentation Consistency | 55 (52–59) | Initially adopted the within-job build-once loop; GH-169 reverted it after neutral hosted timing. No job dependency changes. |
| Quality Coverage and Critical Paths | 858 (796–1127) | Retain isolated instrumented collection; it is the last required child in all three samples. |

Developer critical path has median 903 seconds (846–1141). A Linux monolith
would serialize currently parallel outcomes. The samples show opportunities for
shared setup, but do not quantify compatible compilation saved, artifact transfer
cost, or a non-worsening critical path after such a merge. Reject the merge and
cross-job executable sharing for this delivery. Keep GH-166's separate Cargo
classes and mandatory freshness checks; do not use coverage builds for ordinary
tests, contracts, release or performance claims. Job count is unchanged.

## Validation

Exact commands, results and local evidence hashes are retained in
[the validation record](https://github.com/musutrade/Harness-Gate/blob/d9a932b23dcfea709175285b594ca200450a9971/docs/quality/ci-topology/repeated-work-validation.json). Local Cargo commands use
`CARGO_TARGET_DIR=$PWD/target/gh-167` to keep all build writes in this workspace.
The checkout has no `.harness-gate/flow.toml`; project-local `harness-gate config
check` and `harness-gate verify --profile ci --all` are not applicable. No generic
configuration was created. Full hosted aggregate acceptance and performance
comparison remain with the controller and subsequent OpenSpec tasks.
