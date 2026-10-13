# Arc-Admin shadow parity matrix

Generated from retained evidence by `python3 docs/dogfood/arc-admin/shadow/reproduce.py --write`.

Full profile, all source. Every row remains blocking when selected. Harness-Gate rejected configuration before dispatch; NOT_RUN is neither PASS nor a command failure. Runtime parity is blocked. Validation results do not represent workflow/lifecycle state.

| Gate | Explicitly required | Arc-Admin | Harness-Gate | Discrepancies |
| --- | --- | --- | --- | --- |
| secret scan | always | PASS | NOT_RUN: configuration rejected | HG-CAP-001 |
| architecture audit | always | PASS | NOT_RUN: configuration rejected | HG-CAP-001 |
| backend.format | yes | PASS | NOT_RUN: configuration rejected | HG-CAP-001 |
| backend.clippy | yes | PASS | NOT_RUN: configuration rejected | HG-CAP-001 |
| backend.compile | yes | PASS | NOT_RUN: configuration rejected | HG-CAP-001 |
| backend.tests | yes | PASS | NOT_RUN: configuration rejected | HG-CAP-001 |
| frontend.lint | yes | PASS | NOT_RUN: configuration rejected | HG-CAP-001 |
| frontend.format | yes | PASS | NOT_RUN: configuration rejected | HG-CAP-001 |
| frontend.tests | yes | PASS | NOT_RUN: configuration rejected | HG-CAP-001 |
| frontend.e2e | yes | PASS | NOT_RUN: configuration rejected | HG-CAP-001 |
| frontend.fullstack-smoke | yes | PASS | NOT_RUN: configuration rejected | HG-CAP-001 |
| frontend.build | yes | PASS | NOT_RUN: configuration rejected | HG-CAP-001 |
| workflow.hook-syntax | yes | PASS | NOT_RUN: configuration rejected | HG-CAP-001 |
| workflow.project-init-tests | yes | PASS | NOT_RUN: configuration rejected | HG-CAP-001 |
| workflow.framework-release-config | no (still blocks) | PASS | NOT_RUN: configuration rejected | HG-CAP-001 |
| workflow.framework-upgrade-tests | no (still blocks) | PASS | NOT_RUN: configuration rejected | HG-CAP-001 |
| workflow.template-quality | yes | PASS | NOT_RUN: configuration rejected | HG-CAP-001 |
| workflow.template-quality-tests | yes | PASS | NOT_RUN: configuration rejected | HG-CAP-001 |
| workflow.observability-config | yes | PASS | NOT_RUN: configuration rejected | HG-CAP-001 |
| workflow.tracing-config | yes | PASS | NOT_RUN: configuration rejected | HG-CAP-001 |
| workflow.production-deployment-config | yes | PASS | NOT_RUN: configuration rejected | HG-CAP-001 |
| workflow.audit-retention-config | yes | PASS | NOT_RUN: configuration rejected | HG-CAP-001 |
| workflow.api-generation | yes | PASS | NOT_RUN: configuration rejected | HG-CAP-001 |
| workflow.supply-chain-config | yes | PASS | NOT_RUN: configuration rejected | HG-CAP-001 |
| workflow.format | yes | PASS | NOT_RUN: configuration rejected | HG-CAP-001 |
| workflow.clippy | yes | PASS | NOT_RUN: configuration rejected | HG-CAP-001 |
| workflow.tests | yes | PASS | NOT_RUN: configuration rejected | HG-CAP-001 |

| Dimension | Arc-Admin | Harness-Gate | Discrepancies |
| --- | --- | --- | --- |
| component selection | Observed scope --all and full report select backend, frontend, workflow. | NOT_COMPUTED: production config rejection precedes scope; declaration parity is not observed selection. | HG-CAP-001 |
| PostgreSQL service | Observed postgres:16-alpine, loopback 127.0.0.1:32768, successful pg_isready and TCP connection; backend tests and real full-stack smoke PASS. The specific owned container is absent after verify. Source service manager caches the shared service during sequential execution. | NOT_RUN: no service dispatch or cleanup observed. No runtime equivalence claim. | HG-CAP-001 |
| environment isolation | DATABASE_URL and TEST_DATABASE_URL unset at invocation. Original service injects TEST_DATABASE_URL; both database-consuming steps declare DATABASE_URL removal. Native task_can_remove_an_inherited_environment_variable and task_runs_in_an_isolated_session tests PASS; backend API and smoke PASS. Ambient database rejection unit tests PASS. No adversarial environment run was added (task 6). | NOT_RUN: preserved declarations are not runtime evidence. | HG-CAP-001 |
| diagnostics and validation readiness | Per-gate PASS, parsed result counts and overall TEST_SUMMARY: PASS; intentional panic/failure output belongs to passing negative unit tests, not a failed gate. | E1000 wraps HGCFG-SHARED-SERVICE with source locations, shared resource, conflicting step and dependency/separate-resource help. No gate result. Structural/binding validation success did not establish production readiness. | HG-CAP-001, HG-UX-001 |
| reports and artifacts | Retained scope.json, secret_scan.json, review_context JSON/Markdown, test_result JSON/Markdown and all 25 native command logs. Generated Playwright/build file hashes and sizes are inventoried separately; reproducible outputs are not all bundled. | No reports emitted. Post-Arc scope/verify in both variants left all Arc report hashes unchanged; existing reports must not be attributed to Harness-Gate. | HG-CAP-001 |
| generic quality | Traditional gate PASS is not generic coverage/CRAP/contract evidence. | NOT_RUN before quality collection. No stricter-quality discrepancy observed. GH-203 authenticated baseline/collector prerequisites remain unprovisioned; not reached and not claimed passed. | HG-CAP-001 |

See [observation and classifications](observation.json) for source provenance, commands, service/environment limits, diagnostics and artifact hashes.
