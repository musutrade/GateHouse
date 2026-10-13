# Tasks: Dogfood Harness-Gate on Arc-Admin

This change includes an explicit normative Engineering Policy delta for project-owned validation boundaries. Existing CRAP/coverage/debt/ratchet/fail-closed/measurement-series/Rust-authority semantics remain unchanged.

## 1. Freeze durable architecture policy

- [ ] 1.1 Add the project-owned validation principle to `docs/engineering-policy.md`.
- [ ] 1.2 Document the three extension layers: command hooks, structured result adapters, and quality collector plugins.
- [x] 1.3 Add a regression/consistency guard ensuring application test frameworks are not required as generic-core special cases merely to execute project-owned commands.
- [ ] 1.4 Record the decision in ADR-0049 and strict-validate this OpenSpec.

## 2. Freeze Arc-Admin assurance baseline

- [x] 2.1 Inventory every existing required Arc-Admin step, component, scope rule, service, parser, timeout, environment rule and CI route.
- [x] 2.2 Record current `cargo flow` hook/full behavior and required delivery semantics.
- [x] 2.3 Capture current self-hosted CI cost/timing and evidence/artifact behavior as the before state.
- [x] 2.4 Mark project-specific E2E/API/full-stack/generation/deployment checks explicitly as project-owned validations, not Harness-Gate missing built-ins.

GH-201 evidence: [frozen source/inventory, semantics, CI timing/artifacts and ownership](../../../docs/dogfood/arc-admin/README.md), reproduced by `python3 docs/dogfood/arc-admin/reproduce.py` and its three passing quality-script tests. Task 1.3 is exercised by the passing `project_owned_runner_replacement_preserves_generic_command_gate` integration test (two unknown runners, success/failure, logs and sealed evidence) and the Engineering Policy anchors in documentation consistency. Strict validation: `openspec validate dogfood-harness-gate-on-arc-admin --strict`. This baseline evidence covers the task IDs authorized by GH-201; it does not establish migration, shadow parity or acceptance of the complete proposal.

## 3. Import/migrate the execution plane

- [x] 3.1 Design and implement a deterministic migration/import path for structurally compatible `.arc-flow/flow.toml` execution configuration.
- [x] 3.2 Preserve aliases, components, scope, services, parsers, profiles, required steps, dependencies, environment handling and timeouts where semantics match.
- [x] 3.3 Fail visibly on unsupported or lossy mappings; never silently drop a required Arc-Admin blocker.
- [x] 3.4 Measure manual migration effort and configuration duplication as product UX metrics.

GH-202 evidence: [deterministic import, loss detection, full declaration parity and UX counts](../../../docs/dogfood/arc-admin/import/README.md), [actual local validation results and limitations](../../../docs/dogfood/arc-admin/import/validation.json). Five Rust CLI import tests pass in the 385-test nextest run; independent Python fixtures compare all 25 blockers and every preserved source value against GH-201. Regenerated TOML/report fixtures are byte-identical. Strict OpenSpec validation passes. Runtime shadow parity, Required Quality Aggregate CI and acceptance of the complete proposal are not claimed by these task checkboxes.

## 4. Configure Arc-Admin generic quality

- [x] 4.1 Add Arc-Admin Harness-Gate quality configuration for Angular frontend, Rust backend and relevant component relationships.
- [x] 4.2 Bind the accepted Rust coverage/risk/CRAP series and trusted baseline provider without changing thresholds or lineage semantics.
- [x] 4.3 Keep Angular/TypeScript CRAP explicitly unsupported unless separately certified.
- [x] 4.4 Keep E2E/API/smoke/generation checks as execution hooks rather than fake quality collectors.

GH-203 evidence: [separate quality configuration and trust prerequisites](../../../docs/dogfood/arc-admin/quality/README.md), [actual validation results](../../../docs/dogfood/arc-admin/quality/validation.json). The quality-binding validator accepts the imported flow plus quality bindings and rejects four binding-loss cases; GH-204 separately exercises complete production loading. Five Python regressions compare unchanged packs to certified sources, preserve all 25 execution hooks, and assert Angular CRAP remains unsupported. JSON Schema and strict OpenSpec validation pass; the existing certified workflow acceptance records 12 validated cases. These checkboxes do not claim Arc-Admin runtime parity, hosted Required Quality Aggregate success or acceptance of the complete proposal.

## 5. Run observation and shadow parity

- [x] 5.1 Run Arc-Admin's current `cargo flow` and Harness-Gate against equivalent source states without transferring merge authority.
- [x] 5.2 Compare selected components and all required traditional gate outcomes.
- [x] 5.3 Compare PostgreSQL service behavior, environment isolation, diagnostics, reports and artifacts.
- [x] 5.4 Classify every discrepancy as Arc-Admin issue, Harness-Gate capability gap, Harness-Gate UX gap or expected stricter generic-quality difference.

GH-204 evidence: [reproducible shadow matrix and classifications](../../../docs/dogfood/arc-admin/shadow/README.md), [actual validation results](../../../docs/dogfood/arc-admin/shadow/validation.json). Arc-Admin at the frozen/current revision passed all 25 blockers and both prelude checks. Both unchanged Harness-Gate variants were invoked on the same tracked source and rejected by complete production configuration validation before scope or dispatch. Every unavailable runtime comparison is explicitly NOT_RUN and linked to HG-CAP-001; the narrower migration-readiness signal is HG-UX-001. PostgreSQL availability/cleanup, native environment-isolation tests, diagnostics and hashed reports/artifacts are retained with their observation limits. These checkboxes establish completed observation/classification, not successful runtime parity, authority transfer, hosted Required Quality Aggregate success or acceptance of the whole proposal. Tasks 6–9 remain pending.

## 6. Exercise controlled negative scenarios

- [x] 6.1 Prove a required project-owned E2E/API/smoke command failure blocks through the generic command-hook path.
- [x] 6.2 Prove required collector/evidence failure fails closed.
- [x] 6.3 Prove stale/incompatible baseline cannot reset ratchet/debt lineage.
- [x] 6.4 Prove accepted Rust CRAP regressions block and Angular unsupported CRAP remains non-fabricated.
- [x] 6.5 Prove hook/full/ci omissions remain honest and do not become fake PASS.

GH-205 evidence: [controlled negative corpus and replay](../../../docs/dogfood/arc-admin/negative/README.md), six command receipts and seventeen synthetic quality receipts, with [actual validation](../../../docs/dogfood/arc-admin/negative/validation.json). These fixtures preserve project-owned application logic and prove the boundary failures; they do not establish Arc-Admin runtime parity or accept the entire proposal.

## 7. Evaluate CI topology and cost

- [x] 7.1 Measure shadow-mode added wall/runner cost on Arc-Admin's self-hosted CI. — Three bounded self-hosted full-workload pairs are retained in `docs/dogfood/arc-admin/cost/selfhosted.md`; complete-quality and original-topology savings remain unknown and authority transfer stays blocked.
- [x] 7.2 Identify duplicate command execution and duplicate authoritative measurement collection.
- [x] 7.3 Define a no-duplication authority-transfer topology that preserves current assurance.
- [x] 7.4 Do not delete existing CI/arc-flow gates until replacement ownership and fail-closed behavior are evidenced.

GH-206 evidence: [before/shadow/target cost and ownership analysis](../../../docs/dogfood/arc-admin/cost/README.md), [reproducible 25-command / nine-capability ledger](../../../docs/dogfood/arc-admin/cost/report.json), and [three bounded self-hosted pairs](../../../docs/dogfood/arc-admin/cost/selfhosted.md). The final GH-206 series supersedes the earlier lack of paired measurements: every shadow invocation passes all 27 traditional results and fails closed on missing native quality state. Tasks 7.2–7.4 preserve all existing gates and make the target conditional on trusted quality provisioning and complete assurance evidence. Complete-quality cost and original-topology savings remain unknown; the bounded series does not authorize transfer or accept the complete proposal.

## 8. Product gap remediation

- [x] 8.1 Fix or separately track any Harness-Gate capability gap revealed by Arc-Admin rather than weakening Arc-Admin.
- [x] 8.2 Fix or separately track migration/configuration/diagnostic UX gaps that make compatible project validation unnecessarily difficult.
- [x] 8.3 Add regression fixtures/tests for every generic product gap fixed through dogfood.
- [x] 8.4 Re-run parity and negative evidence after any Harness-Gate remediation.

  Evidence: [GH-207 remediation ledger](../../../docs/dogfood/arc-admin/remediation/README.md), [validation](../../../docs/dogfood/arc-admin/remediation/validation.json) and hashed rerun receipts. All 27 application results pass in each of three local runs; both Harness-Gate paths retain the missing-native-state block. Six CLI controls and seventeen signed quality cases retain expected outcomes. Remaining native integration, host/hook/routing and complete cost evidence are explicitly tracked with rationale in [GH-215](https://github.com/musutrade/Harness-Gate/issues/215); task 9 and proposal-wide acceptance remain pending.

## 9. Authority-transfer recommendation and closure

- [x] 9.1 Produce an evidence-backed recommendation on whether Arc-Admin can transfer required workflow authority from `cargo flow` to Harness-Gate.
- [x] 9.2 If transfer is not yet safe, keep shadow mode and list exact blockers; do not force closure.
- [ ] 9.3 If transfer is safe, create a separate follow-up change for removal/consolidation of old Arc-Admin workflow infrastructure. — Not applicable to the current no-transfer decision; safe-transfer condition is false and no removal change is authorized.
- [ ] 9.4 Retain final migration effort, parity, negative, cost, rollback and product-gap evidence; strict-validate and close this dogfood change only when all claims are supported. — Evidence retention and strict validation are recorded below; proposal closure remains blocked, so this combined task is not marked complete.

GH-208 evidence: [no-transfer recommendation, final evidence index, exact blockers
B1–B5 and rollback boundary](../../../docs/dogfood/arc-admin/decision/README.md),
[actual validation](../../../docs/dogfood/arc-admin/decision/validation.json).
Retained GH-207 semantic replay confirms 27 passing traditional results per engine,
six command controls and seventeen synthetic quality cases, with both Harness-Gate
full workflows still blocked on missing native state. Self-hosted receipt replay
confirms bounded cost evidence, not complete-quality success or target savings.
GH-215 tracks integration/host/routing/cost remediation. Existing gates, authority,
quality policy and lineage remain intact; Result is validation evidence only.
Strict OpenSpec validation passes; full local check results and hosted CI limits
are retained in the validation record. Tasks 1.1, 1.2 and 1.4 and whole-proposal
acceptance remain outside this issue's completion claim. Required Quality Aggregate
remains CI pending at submission; the controller owns acceptance and issue closure.
