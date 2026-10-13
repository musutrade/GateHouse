# Tasks: Integrate Generic Quality into Project Workflow

All tasks inherit `docs/engineering-policy.md` and the accepted CI topology. CRAP/coverage/debt/ratchet/fail-closed/measurement-series/Rust-authority semantics are unchanged unless a separate explicit normative policy delta is approved.

## 1. Define quality.toml v1 and cross-plane validation

- [x] 1.1 Define a language-neutral `.harness-gate/quality.toml` v1 schema/model for project/components, source/artifact boundaries, subjects/discovery, relationships, collector bindings, capability/series expectations, policy bindings, profile participation, baseline provider, and report intent.
- [x] 1.2 Enforce authority constraints: collectors cannot declare final requiredness, thresholds, ratchets, aggregate PASS/FAIL, or release authority.
- [x] 1.3 Extend `config check/print` and schema/docs tooling to validate flow/quality cross-references, unresolved components/subjects/relationships, incompatible series, missing required producers, invalid profile participation, and baseline/ratchet contradictions.
- [x] 1.4 Preserve backward compatibility for repositories without `quality.toml`; no implicit new collectors or policy are imposed on legacy flow-only projects.

GH-179 local evidence (2026-09-09): `cargo nextest run --manifest-path tools/harness-gate/Cargo.toml --locked` passed 344 tests, including the new config/schema/CLI authority and cross-reference cases and the frozen Rust/Python comparison suites. `cargo fmt --manifest-path tools/harness-gate/Cargo.toml -- --check` and `cargo clippy --manifest-path tools/harness-gate/Cargo.toml --all-targets -- -D warnings` passed. `python3 -m unittest discover -s tools/quality/tests -v` passed 328 tests; `python3 tools/quality/docs_consistency.py --output target/quality/docs-consistency.json` passed, validating both schemas, the quality example and all legacy presets. Cargo-dependent checks used `CARGO_TARGET_DIR=$PWD/target` because the environment default `/home/gem/cargo-target` is read-only. Full local logs are in `target/quality/gh-179/`. Root `harness-gate config check` and `harness-gate verify --profile ci --all` are not applicable: this source checkout has no `.harness-gate/flow.toml`. `openspec validate integrate-generic-quality-into-project-workflow --strict` passed. Hosted Required Quality Aggregate and controller acceptance remain pending; tasks 2 onward are not implemented by GH-179.

GH-179 CI repair evidence (2026-09-09, PR #188): the aliased-root regression failed before root canonicalization and passed afterward, including rejection of escaping child symlinks. The required nextest command now passes 345 tests; fmt and clippy pass. The full Python suite passes 329 tests, including the expanded source-analyzer contract; the focused CI-tool suite passes 11 tests after the isolated archive fix. Docs consistency and strict OpenSpec validation pass. The local CI collector passes legacy coverage, production coverage and the critical-path matrix. Config production coverage is 89.33% lines and 87.85% regions against unchanged 80% thresholds. The first local risk attempt failed because its isolated snapshot omitted the schema compilation input; that archive fix and the versioned, same-selection base/head remeasurement are recorded in ADR-0041. The subsequent risk comparison exposed the raw flow-print path missing from the CLI regression; assertions now prove byte-for-byte flow output before and after quality is configured. Fresh instrumented head measurement passes all 345 tests, and comparison against the same-selection, source/hash-verified baseline passes all 527 identities with zero failures. The final head-only measurement command is `PATH="$PWD/target/quality/gh-179/bin:$PATH" python3 target/quality/gh-179/remeasure-head.py`; it retains the already measured unchanged baseline and performs the collector's source/manifest/raw-profile checks for both sides. Evidence is retained under `target/quality/gh-179/repair-*`; hosted Required Quality Aggregate remains pending.

## 2. Compile project configuration to trusted generic inputs

- [x] 2.1 Implement deterministic compilation from validated quality configuration + repository/source/profile/scope state into `harness-project/v1`, `harness-policy/v1`, trusted expected context, roots, and optional selection/mapping/exception inputs.
- [x] 2.2 Define stable identity/digest rules for compiled inputs and reject mixed/stale source, config, tool, or artifact identity.
- [x] 2.3 Add equivalence tests proving direct `quality evaluate` and the compiled path produce identical generic Rust decisions/reports for equivalent inputs, including fail-closed negative cases.

GH-180 local compiler evidence (2026-09-09): the focused nextest CLI integration test passes all 50 retained compilation/equivalence cases with zero unexplained mismatches. Twenty direct/compiled cases compare exact report bytes and exit status, including Rust passing/failing coverage, missing/stale/malformed/duplicate evidence, unsupported capability, subject policy expansion, discovery, multiple subjects, relationships, optional mappings/exceptions and baseline ratchets. Twenty-five stale/mixed compiler cases fail closed and remove stale outputs; four output-alias cases preserve inputs; a changed run context changes the digest. Repeated compilation and reordered subject sets preserve output bytes. Unknown ecosystem/custom collector/series fixtures use the same compiler path, with supported metrics passing and a custom unsupported metric rejected by the existing core. All 346 Rust tests and 329 Python tests pass; fmt, clippy, docs consistency and strict OpenSpec validation pass. Production coverage passes every required boundary against the unchanged 80% threshold (config lines 90.26%; aggregate lines 88.16%). Full-suite results and local measurement/delivery limitations are recorded in `docs/quality/gh-180/validation.md`. See `docs/quality-compilation.md` and ADR-0042. Root `config check` and `verify --profile ci --all` are not applicable because `.harness-gate/flow.toml` is absent. Hosted Required Quality Aggregate/controller acceptance remain pending; tasks 3 onward are not marked complete.

## 3. Orchestrate collectors through the trusted adapter boundary

- [x] 3.1 Add project-level collector binding/orchestration over the existing signed out-of-process adapter protocol, including compatible protocol/version, executable/package identity, roots, limits, component/subject selection, and expected capabilities.
- [x] 3.2 Validate capability honesty and normalized evidence: supported/unsupported/not-collected/error remain distinct; no synthetic CRAP or favorable values are invented.
- [x] 3.3 Fail closed for required collector launch/signature/protocol/timeout/crash errors, missing expected capability, malformed evidence, incompatible series, identity mismatch, stale/mixed artifacts, or duplicate/conflicting authoritative producers.
- [x] 3.4 Retain low-level `adapter run` as an advanced/debug protocol interface; project users should not manually construct adapter requests for normal `verify`.

GH-181 local evidence (2026-09-09): `cargo nextest run --manifest-path tools/harness-gate/Cargo.toml --locked` passed 352 tests. The six collector tests also passed after extending the unknown-ecosystem fixture with an arbitrary capability: configured supported contracts succeed, while unsupported metric contracts launch and fail closed at the released core. Tests cover all capability availability states, signed request/response identity and limits, malformed/duplicate JSON, missing/conflicting claims, multiple producers, source/config/artifact tampering, and CLI publication. A source guard rejects closed language dispatch. `cargo fmt --manifest-path tools/harness-gate/Cargo.toml -- --check` and `cargo clippy --manifest-path tools/harness-gate/Cargo.toml --all-targets -- -D warnings` passed. `python3 -m unittest discover -s tools/quality/tests -v` passed 329 tests; the source-measurement suite passed 10 tests after the source selection update. `python3 tools/quality/docs_consistency.py --output target/quality/docs-consistency.json` passed. Cargo-dependent commands use `CARGO_TARGET_DIR=$PWD/target`: the environment default `/home/gem/cargo-target` is read-only. Root `harness-gate config check` and `harness-gate verify --profile ci --all` are not applicable because `.harness-gate/flow.toml` is absent. Evidence logs are retained under `target/quality/gh-181/`; the binding/protocol and measurement review are recorded in `docs/quality-collectors.md` and ADR-0043. `python3 tools/quality/coverage.py --production --output target/quality/gh-181/production.json` passed with 88.37% lines, 85.64% regions and all blocking boundaries green. This completes the collector stage only: normal `verify` integration remains task 5.1, and hosted Required Quality Aggregate/controller acceptance and the rest of this proposal remain pending.

GH-181 CI repair (2026-09-09): PR #190 at `34469e034cc6056f89f4631c5513dbd3c288174a` failed three collector tests on macOS because the fixture signed a noncanonical temporary artifact root. A symlinked-workspace regression reproduced the exact `collector roots/selection/capability request mismatch` on Linux before the fix. The fixture now signs the compiler's canonical artifact root; production validation remains unchanged. The required local commands above were rerun: nextest passed 353 tests (including the regression and all three previously failing cases), fmt and Clippy passed, Python unittest passed 329 tests, and docs consistency passed. Cargo-dependent commands again used the workspace target. Project-local config check and CI verify remain not applicable because `.harness-gate/flow.toml` is absent. Logs: `target/quality/gh-181-repair/`. Native macOS execution and the new hosted aggregate remain pending CI; no further proposal tasks are checked off.

## 4. Implement trusted baseline providers

- [x] 4.1 Implement a deterministic Git base-ref/merge-base provider that materializes exact source identity without mutating the working tree and supplies compatible base context required by the Rust evaluator.
- [x] 4.2 Implement/define retained CI artifact baseline resolution with exact base commit/config/tool/series identity plus manifest/hash validation under the accepted CI artifact trust model.
- [x] 4.3 Enforce existing debt/ratchet lineage semantics: missing/incompatible baseline cannot silently reset debt or create a favorable fresh baseline.
- [x] 4.4 Add positive/negative tests for rename/move lineage, missing base, incompatible series, stale artifact, changed config/tool identity, and optional-vs-required baseline policy.

Validation: [GH-182 evidence](../../../docs/quality/gh-182/validation.md) records the
39-case baseline corpus, 354 passing Rust tests, 329 passing Python tests and local
checks. Hosted Required Quality Aggregate remains pending; later tasks are unchanged.

## 5. Integrate generic quality into verify and unified reporting

- [x] 5.1 Extend `harness-gate verify` phases to load quality config, resolve selection, run traditional gates, orchestrate applicable collectors, validate evidence, resolve baseline, compile trusted inputs, invoke the released Rust generic core, and emit one final status.
- [x] 5.2 Ensure traditional step exit codes cannot override generic policy authority; any required traditional or generic failure blocks final verify.
- [x] 5.3 Define/extend stable machine reporting to link execution outcomes, collector/capability state, evidence/artifact identities, component/local results, cross-component contracts, baseline/ratchet/debt, exceptions, authoritative project report, and final combined status.
- [x] 5.4 Improve human diagnostics, including CRAP failures with subject, base/head CRAP, threshold/ratchet state, supporting evidence, and remediation context.
- [x] 5.5 Preserve direct `quality evaluate` as the low-level advanced interface and prove report/decision equivalence with the verify path.

Validation: [GH-183 evidence](../../../docs/quality/gh-183/validation.md) records
360 passing Rust tests, the Python suite and architecture guard, full direct CLI
report/decision equivalence, signed unknown-ecosystem collection with retained
baseline, and independent execution/audit/quality blocking. Hosted Required
Quality Aggregate is pending; tasks 6 onward and whole-change acceptance remain
unchanged. See [ADR-0045](../../../docs/adr/0045-quality-verification-composition.md).

## 6. Enforce profile semantics and quality cost boundaries

- [x] 6.1 Define explicit hook/full/ci quality participation. Hook may omit expensive full coverage/CRAP collection but must not fabricate full-quality PASS.
- [x] 6.2 Make certified required Rust CRAP/risk/coverage participate by default in applicable `full`/`ci` preset policy without changing accepted thresholds or series.
- [x] 6.3 Keep uncertified Angular/TypeScript CRAP explicitly unsupported; do not derive an invented CRAP series from unrelated metrics.
- [x] 6.4 Ensure CI consumes existing authoritative retained quality evidence where available rather than launching duplicate equivalent collection; record any unavoidable new cost using the accepted CI topology evidence model.

GH-184 local evidence (2026-09-09): 373 Rust tests and 335 Python tests passed;
fmt, clippy, documentation/schema consistency and strict OpenSpec validation
passed. Metadata-driven complete/partial assurance, honest omitted measurements,
certified Rust policy/series parity, native TypeScript unsupported CRAP, an
unknown ecosystem with cheap/expensive/custom profile selections, and all-retained
and mixed-producer reuse are covered. Invalid retention fails before fallback.
The architecture guard covers generic configuration and profile implementation.
See `docs/quality/gh-184/validation.md`, `docs/quality-profiles.md` and ADR-0046 for
actual commands, migration implications, trust constraints and CI cost boundaries.
The accepted CI topology is unchanged; no new hosted job or collection is added.
Hosted timing and Required Quality Aggregate/controller acceptance remain pending.
Root config check and ci verify are not applicable without `.harness-gate/flow.toml`.
This checks off only section 6; preset generation and hosted wiring remain below.

## 7. Upgrade presets and migration UX

- [x] 7.1 Upgrade `rust-api` to generate coherent flow + quality configuration with accepted Rust quality capabilities and full/CI CRAP policy.
- [x] 7.2 Upgrade `angular-only` with certified Angular/TypeScript capabilities while keeping CRAP unsupported.
- [x] 7.3 Upgrade `angular-rust-postgres` with frontend/backend components, ecosystem-specific collectors, relationship/contract modeling, service/execution separation, and one project aggregate.
- [x] 7.4 Decide/document backward-compatible behavior for `generic` preset and existing flow-only repositories; provide explicit migration/enablement guidance instead of silently imposing policy.
- [x] 7.5 Extend preset/config/docs consistency tests so generated flow + quality files cross-validate and retain Engineering Policy anchors.

GH-185 local evidence: 378 nextest tests and 341 Python tests passed; formatting,
Clippy, docs consistency (all four generated presets plus opt-in migration), and
strict OpenSpec validation passed. Unknown-ecosystem composition and generic
plumbing guards pass without schema/compiler/verifier ecosystem branches.
See [validation and limitations](../../../docs/quality/gh-185/validation.md) and
[ADR-0047](../../../docs/adr/0047-composable-quality-preset-packs.md).
Only tasks 7.1–7.5 are checked off here. Root config/CI verification is not
applicable without project-local flow.toml; hosted Required Quality Aggregate
and sections 8–9 remain pending, not accepted by these local checks.

## 8. CI integration and hosted acceptance

- [x] 8.1 Integrate verify/generic orchestration with the optimized CI topology while preserving pinned tools, one-owner measurement series, immutable artifact validation, lightweight Required Quality Aggregate, and current native platform assurance.
- [x] 8.2 Prove no duplicate authoritative coverage/risk/CRAP collection is introduced for CI merely because verify now orchestrates generic quality.
- [x] 8.3 Capture hosted timing/runner-work impact for the new integration and reject/rework execution choices that add avoidable cost without assurance benefit; do not reintroduce previously reverted ineffective compiled-cache/build-once optimizations without new evidence. GH-186 run `34419625673`, attempt 1, head `024f775904eb6bd50639092d3779d9e2f8dad035`: [reproduced timing](../../../docs/quality/gh-187/hosted-prerequisite-timing.json) and [cost/assurance acceptance](../../../docs/quality/gh-187/hosted-closure.md).
- [x] 8.4 Run semantic parity/negative acceptance across Rust, Angular/reference, and mixed Angular+Rust project shapes; retain exact evidence and Required Quality Aggregate success. GH-186 run `34419625673`, attempt 1: [Required Quality Aggregate job `102695650692`](https://github.com/musutrade/Harness-Gate/actions/runs/34419625673/job/102695650692) and every required child succeeded; the [validated hosted receipt](../../../docs/quality/gh-187/hosted-prerequisite-summary.json) retains the same 12 cases and 40 negatives, including the unknown ecosystem.

GH-186 local evidence (2026-09-09): the existing required Linux Test job now
validates and uploads an identity-bound configured acceptance receipt. Twelve
Rust, Angular/reference, mixed and unknown-ecosystem cases have exact direct Rust /
verify project-report parity; forty identity/artifact/source negatives fail closed.
Producer ledgers show one launch per case and zero reuse/fallback launches after
removing the executable. Capability/profile/project metadata drives the same
orchestration, validation, aggregate and report path with no ecosystem jobs or
language dispatch. Pinned tools, production measurement ownership/seals, aggregate
allowlist and existing native platform assurance are unchanged. Full local
nextest (379 tests), Python (346 tests), fmt, Clippy and docs consistency pass;
strict OpenSpec validation passes. See `docs/quality/gh-186/validation.md`, its
retained receipt/log manifest and ADR-0048 for exact commands and limitations.
Root config check / CI verify remain not applicable without project-local flow.toml.
At GH-186 submission, tasks 8.3–8.4 awaited the controller-owned hosted comparison
and Required Quality Aggregate. GH-198 now retains their final acceptance above;
historical CI-topology cohorts remain comparison inputs, not new-head acceptance.

## 9. Documentation, CLI positioning, and closure

- [x] 9.1 Update README/quick start/architecture/configuration/CLI help so `init -> verify -> one project decision` is the primary product story and `quality evaluate` / `adapter run` are clearly advanced interfaces.
- [x] 9.2 Document quality.toml schema, capability states, collector-vs-policy authority, baseline providers, profile behavior, CRAP support boundaries, artifact trust, migration, and troubleshooting.
- [x] 9.3 Strict-validate this OpenSpec and run all required repository quality checks; record final equivalence, fail-closed, hosted CI, performance/cost, and rollback evidence. GH-187 run `34422630969`, attempt 1, head `0d2dfc23481cde02c3adc3f6659d00ca001ce3d8`: [Required Quality Aggregate job `102704478218`](https://github.com/musutrade/Harness-Gate/actions/runs/34422630969/job/102704478218) and all required children, including native macOS/Windows, succeeded. [Final reconciliation](../../../docs/quality/gh-187/hosted-closure.md) and [GH-198 local validation](../../../docs/quality/gh-198/validation.json) retain exact evidence.
- [x] 9.4 Keep Java/Python/third-ecosystem adapters, compat deprecation, risk-driven conditional hosted platforms, and real application dogfood outside this change; create separate follow-ups only after acceptance.

GH-187 documentation evidence (2026-09-10): README/quick start, architecture,
configuration and CLI help now lead with `init -> verify -> one project decision`;
advanced interfaces and all configuration/trust/certification/migration boundaries
are documented in `docs/quality-workflow.md`. Closure explicitly treats ecosystem,
language and framework identity as configuration/pack data across every generic
layer. The retained unknown-ecosystem matrix and companion pack/baseline/profile
tests demonstrate the collector + capability/policy pack + certification extension
path, without language dispatch or generic-core redesign. Future Vue + Go +
PostgreSQL composition is distinguished from current bounded reference presets.
See `docs/quality/gh-187/validation.md` and its retained command/receipt manifest
for actual validation, equivalence, fail-closed, hosted prerequisite, timing and
rollback evidence. At GH-187 submission, task 9.3 awaited that head's required
hosted checks and Required Quality Aggregate; prior hosted evidence alone did not
establish current-head acceptance. GH-198's final reconciliation retains that
current-head acceptance. No deferred
adapters, deprecation, platform policy, dogfood or follow-up issues are added.

GH-198 final reconciliation (2026-09-10): retained both controller-confirmed
hosted heads and reproduced normalized timing and the 12-case/40-negative
acceptance receipts. Required local nextest (379 tests), Python (346 tests), fmt,
Clippy, docs consistency, strict OpenSpec validation and diff checks pass.
Root config check / CI verify remain not applicable without project-local
flow.toml. The final record explicitly accepts measured cost against assurance,
without a causal speedup claim, and preserves configuration/pack-driven extension,
native requiredness and the unchanged fail-closed aggregate. All implementation
tasks have acceptance evidence; formal closure follows green GH-198 CI and the
controller's merge. This issue changes only governance/evidence records.
