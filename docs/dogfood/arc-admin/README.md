# Arc-Admin assurance before state

> Historical source/evidence snapshot: recorded file hashes in this directory are preserved. For the completed current native integration, released Core fixes and scoped cost observation, see the [2026-09-13 acceptance](../../quality/arc-native-20260913/README.md) and [release status](../../release-status.md).

GH-208 [final recommendation and rollback boundary](decision/README.md): **no
authority transfer**. Retain `cargo flow` and bounded shadow mode; native quality,
host/hook/CI parity and complete cost acceptance remain blocked. The complete
OpenSpec remains active.

Follow-on records: [execution import (GH-202)](import/README.md), [generic quality configuration (GH-203)](quality/README.md), [shadow observation (GH-204)](shadow/README.md) [controlled negatives (GH-205)](negative/README.md) and [bounded self-hosted cost/parity trials (GH-206)](cost/selfhosted.md). These preserve this frozen baseline; traditional execution now matches, while full-quality integration and authority transfer remain pending.

This freezes OpenSpec `dogfood-harness-gate-on-arc-admin` tasks 2.1–2.4 for GH-201, before import, shadow execution or transfer of authority. The source is **musutrade/arc-admin at `9982ed556eaf997910824d7b682946147c81a16a`**, the main-branch merge of PR #30 on 2026-08-24. Evidence was captured read-only through GitHub on 2026-09-10. No Arc-Admin checkout was executed or modified. Source inspection confirms execution semantics; historical Actions records provide observed CI outcomes and timing. This is not a fresh local Arc-Admin verification or parity claim.

The assurance boundary includes every selected command, both prelude checks, environment isolation, scope selection and all three CI workflows. Nothing here authorizes removing a blocker. Harness-Gate's CRAP, coverage, debt, ratchet, fail-closed behavior and Rust policy authority are unchanged. See [ADR-0049](../../adr/0049-project-owned-validation-extension-boundary.md), [Engineering Policy §7](../../engineering-policy.md#7-project-owned-validation-and-extension-boundaries) and the [OpenSpec tasks](../../../openspec/changes/dogfood-harness-gate-on-arc-admin/tasks.md).

## Reproduce and inspect

GH-202 adds the [deterministic execution import, parity inventory and UX metrics](import/README.md).
The frozen before state below remains unchanged; the import does not establish runtime parity.

- [Source manifest](source-manifest.json): original paths, pinned Git blob IDs, SHA-256 hashes and local `.txt` snapshots. These are inert evidence, not installed configuration.
- [Inventory](inventory.json): complete parsed flow, architecture audit and secret-scanner configuration, all components, profiles, scope patterns, services, parsers, doctor rules, environment declarations and step ownership.
- [Execution table](steps.md): all 25 commands, arguments, working directories, profiles, timeouts, parsers, services and log names.
- [CI capture](ci-capture.json): run/job/step timestamps, conclusions, runner labels and artifact metadata, with exact API endpoints and response status/counts.
- [Cost summary](cost-summary.json): deterministic calculations from that capture.
- [Implementation validation](validation.json): actual GH-201 local checks, resolved environment/test failures and CI limitations.

Run from this repository root with Python 3.11+:

```sh
python3 docs/dogfood/arc-admin/reproduce.py
python3 -m unittest discover -s tools/quality/tests -p test_arc_admin_baseline.py -v
```

The check verifies both SHA-256 and Git blob hashes and reproduces the inventory, execution table and cost summary offline. `--write` regenerates those three derived files from the frozen inputs; it does not fetch a newer baseline. The tests also reject changed source bytes and changed derived inventory/cost evidence. These tests run in the existing quality-scripts route feeding Required Quality Aggregate.

To independently retrieve a source, use GitHub REST `GET /repos/musutrade/arc-admin/contents/{path}?ref=9982ed556eaf997910824d7b682946147c81a16a` for a manifest path, base64-decode `content`, and compare its blob ID and bytes. For Actions, use the captured `/actions/runs/{id}`, `/jobs?per_page=100` and `/artifacts?per_page=100` endpoints; the recorded totals fit on one page. API access requires repository visibility. Historical API availability, retention and artifact expiry can change; the captured metadata is retained for offline reproduction. This is a verifiable source snapshot, not a cryptographically signed provenance attestation.

## Execution and delivery contract

The Cargo alias invokes `run --quiet --locked --manifest-path codex-audit-pipeline/tools/arc-flow/Cargo.toml --`. The pre-commit hook uses `set -eu`, enters the Git root and `exec cargo flow hook`. Configuration defaults to profile `full`, with `hook_profile = "hook"`; there is no `ci` profile at this baseline.

`hook` selects staged changed paths and scans staged secret content. Architecture audit and configured commands still operate on the working tree, not a materialized index. `verify` defaults to full and working-tree scope: unstaged changes, staged changes and untracked files. `--base REF` uses the triple-dot diff `REF...HEAD`; `--all` selects all three components; `--components` validates explicitly requested components. `verify --staged` changes scope selection, but unlike `hook` its secret scan is still invoked with working-tree mode. These distinctions must survive migration or be reported as explicit differences.

The secret scan always runs first. When it passes, architecture audit runs and requires **zero total violations**, including warnings. Either failed prelude suppresses configured commands. Complete audit layer/dependency/forbidden-pattern rules and secret patterns, allowlists and exclusions are retained in `inventory.json` and their original configuration snapshots. All selected configured steps execute sequentially; the baseline has no declared step dependencies, concurrency or retries. An ordinary command or parser failure is recorded and later steps continue; a service setup failure records a failed step and continues. Process setup errors or cancellation can terminate verification before its final summary. Overall success requires every recorded check to pass.

There are **25 full-profile steps and 17 hook-profile steps**. Full alone adds backend compile/tests, frontend unit tests/E2E/full-stack smoke/build, API generation and workflow Clippy. `policy.required_steps` lists 23 IDs and validates their presence; it is not the execution filter. **`workflow.framework-release-config` and `workflow.framework-upgrade-tests` also block when selected**, despite their omission from that list. Preserve all 25 entries in the [execution table](steps.md), not only the policy list.

Arc-Admin's pinned `AGENTS.md` requires **`cargo flow verify --all` before remote delivery**, prohibits bypassing gates and prohibits production databases for tests. Hook success alone does not establish full verification. Its optional LLM review is advisory; it does not replace deterministic verification. No claim is made here about GitHub branch-protection settings, which were not captured.

## Components and scope

Components are `backend`, `frontend` and `workflow`. Matching rules accumulate component membership. Every changed path must match; unmatched paths fail closed. The complete seven-rule mapping is:

| Patterns | Components |
| --- | --- |
| `README.md`, `CHANGELOG.md`, `.gitignore`, `.gitattributes`, `.dockerignore`, `AGENTS.md`, `start.sh`, `deny.toml`, `scripts/**`, `observability/**`, `deployment/**`, `compose.production.yaml`, `FRAMEWORK_VERSION`, `.arc-framework/**`, `.arc-project.json`, `docs/**`, `extracted/**`, `codex-audit-pipeline/README.md`, `codex-audit-pipeline/docs/**`, `.github/**` | workflow |
| `backend/**` | backend |
| `frontend/**`, `.node-version` | frontend |
| `rust-toolchain.toml` | backend, workflow |
| `.arc-flow/**`, `codex-audit-pipeline/tools/arc-flow/**`, `codex-audit-pipeline/hooks/**`, `.cargo/config.toml` | backend, frontend, workflow |
| `backend/src/openapi.rs`, `backend/src/bin/export_openapi.rs`, `docs/openapi.json`, `frontend/ng-openapi-gen.json`, `frontend/src/app/generated/api/**`, `.github/workflows/**` | backend, frontend, workflow |
| `codex-audit-pipeline/.codex/audit.toml`, `codex-audit-pipeline/.codex/secrets.toml`, `codex-audit-pipeline/templates/**` | workflow |

## Services, parsing, timeouts and environment

There is one configured service, `test-postgres`, used by `backend.tests` and `frontend.fullstack-smoke`. It accepts `TEST_DATABASE_URL` or starts `postgres:16-alpine` with an ephemeral loopback host port mapped to 5432, `pg_isready` health checking and the declared synthetic `arc_admin_test` database/user/password. It injects `TEST_DATABASE_URL`. Container ownership and cleanup remain with arc-flow; supplied external databases are not owned containers.

The `isolated-postgres` policy requires a PostgreSQL URL, a database ending in `_test` or `-test`, and a loopback host unless `ARC_FLOW_ALLOW_REMOTE_TEST_DATABASE=1`. It rejects the runtime `DATABASE_URL` both as the same URL and as the same normalized host/port/database (including equivalent loopback hostnames). Both service-using steps remove inherited `DATABASE_URL` before execution. Their project test code retains ownership of migrations and assertions. The full-stack startup script deliberately sets its isolated application's `DATABASE_URL` from the validated test URL.

Two regex parsers strip ANSI escapes, sum capture group 1 and require at least one test even when the executable exits successfully: `rust` uses `(?m)^running ([0-9]+) tests?$`; `angular` uses `Tests\s+([0-9]+) passed`. Rust parsing applies to backend/workflow tests; Angular parsing applies to frontend unit tests. Other project commands use exit status, not invented test counts or quality metrics.

The [execution table](steps.md) freezes every outer step timeout. Overrides are `RUST_TEST_TIMEOUT` (backend tests 600s; workflow tests 120s), `ANGULAR_TEST_TIMEOUT` (180s), and `ANGULAR_BUILD_TIMEOUT` (180s). Service startup defaults to 30s, overridden by `ARC_FLOW_DATABASE_TIMEOUT_SECS`; its image is overridden by `ARC_FLOW_POSTGRES_IMAGE`. Invalid integer overrides fail configuration loading. Task timeouts and cancellation terminate execution; they do not become passes.

Configuration/environment rules also include:

- CLI project-root override precedes `PROJECT_ROOT`; otherwise discovery searches ancestors. CLI config override and `ARC_FLOW_CONFIG` select the flow configuration. Paths and aliases resolve within the project boundary.
- `REPORT_DIR`, then `ARC_FLOW_REPORTS` override reports; `AUDITOR_CONFIG`, then `ARC_FLOW_AUDIT_CONFIG` override audit configuration; `ARC_FLOW_SECRETS_CONFIG` overrides secret configuration. Alias overrides are `ARC_FLOW_BACKEND`, `ARC_FLOW_FRONTEND` and `ARC_FLOW_TOOL_MANIFEST`. Required audit/secret configuration must exist.
- Commands inherit the process environment, apply configured removals and service injections. No additional per-step environment map is declared in this flow. The full-stack project script owns `APP_ENV=test`, `AUTO_MIGRATE=false`, `PORT=18081`, CORS, logging, service name, WebAuthn settings and synthetic `BOOTSTRAP_ADMIN_*` test fixtures; their exact values are frozen in its snapshot.
- Project Playwright configurations own `CI`-dependent `forbidOnly`, retry counts, server reuse, reporters and tracing. Full-stack smoke runs one worker, allows one CI retry, requires fresh backend/frontend servers and declares 300s/120s startup limits within the 420s outer gate. These are project semantics, not an arc-flow retry facility.
- Doctor declares 12 checks: Git, Cargo, Rust, Node, npm, frontend dependencies, runtime database env/file, migration files, optional Git hook configuration, Node version, Git remotes, and optional test database service. The exact required flags, help text and predicates are in the inventory. CI's frontend route additionally runs `doctor --strict --json` with pipefail; this is a separate prerequisite, not an implicit prelude to every verify call.

## CI routes and artifacts

All configured jobs use `self-hosted`. Workflow concurrency groups cancel earlier runs on the same ref. The pinned YAML snapshots preserve exact action versions, commands, conditions, permissions and environment values.

| Workflow / route | Trigger and selection | Timeout / blocking behavior | Evidence |
| --- | --- | --- | --- |
| CI / Detect change scope | PR; push to main. PR base SHA or nonzero valid push `before`; otherwise all. Full-depth checkout, Node setup, `cargo flow scope --base … --json` or `--all --json`; emit comma-separated components. | 10m; downstream component jobs need scope success | Always upload `scope.json` as `scope-report`, ignore missing, 14 days |
| CI / Quality gate | `workflow` selected | 15m; Node setup/npm ci, `cargo flow verify --components workflow` (full default) | Always `quality-reports`, full report directory, ignore missing, 14 days |
| CI / Backend verification | `backend` selected | 20m; CI PostgreSQL service, `cargo flow verify --components backend` | Always `backend-report`, full report directory, ignore missing, 14 days |
| CI / Frontend verification | `frontend` selected | 20m; CI PostgreSQL, Node/npm ci, doctor prerequisites and strict doctor, production npm audit at high severity, Chromium install, `cargo flow verify --components frontend` | Always `frontend-report`, full report directory including doctor JSON, ignore missing, 14 days |
| CI / Dependency review | PR and public repository only | 10m; dependency-review, fail at moderate | Action diagnostics; skipped in captured push |
| CodeQL / JS-TypeScript and Rust matrix | PR, push main, Tuesday 03:41 UTC, manual; public repository only | 30m each, fail-fast false, build-mode none, security-extended queries | CodeQL analysis upload; security-events write; skipped at pinned commit |
| Supply chain security / Rust dependency policy | PR, push main, Monday 02:17 UTC, manual | 15m; RustSec audit; cargo-deny always runs advisories/bans/licenses/sources with `RUSTUP_TOOLCHAIN=1.85.0-x86_64-unknown-linux-musl` | Action logs; existing `RUSTSEC-2023-0071` ignore is explicitly retained (inactive SQLx MySQL dependency per YAML comment) |
| Supply chain security / Backend and frontend container matrix | Same security triggers | 45m each, fail-fast false; production Docker build, Trivy HIGH/CRITICAL, exit 1, ignore unfixed | SPDX SBOM generation runs even after scan failure if build succeeded; upload always, warn if missing, 30 days |

CI PostgreSQL uses `postgres:16-alpine`, explicit host port 5432, the same test database credentials and `TEST_DATABASE_URL`; health checks run every 5s with 10 retries. Frontend CI creates its example `.env`, hook configuration and report directory before doctor. Browser installation expects host system dependencies already provisioned (the YAML documents CentOS). Scope routing avoids unrelated component execution; it does not remove the mandatory local all-components delivery check.

Locally, arc-flow writes mutable `codex-audit-pipeline/.codex/reports`: `scope.json`, `changed_files.txt`, `secret_scan.json`, audit `review_context.json`/`.md`, per-step `logs/*`, and `test_result.json`/`.md` with profile, scope, check outcomes, durations and overall pass; it prints `TEST_SUMMARY`. Error paths can stop before final summaries. CI uploads use `if: always()` but ignore missing reports, so successful upload steps alone cannot prove complete evidence. Playwright HTML/traces live in project frontend output directories and are not explicitly included by those report-directory upload routes. This baseline does not claim immutable/sealed arc-flow artifacts. Captured Actions metadata includes names, byte sizes, digests and expiry; artifact payloads were not downloaded or inspected.

## Observed cost before migration

The samples below are completed historical runs, not newly triggered jobs. All timings are seconds from GitHub timestamps; runner occupancy is the sum of executed job durations, not wall time or billable minutes. Skipped jobs contribute zero, even where their API timestamps are reversed.

| Run | Source / outcome | Created-to-updated | First-job wait | Execution span | Runner seconds | Artifact bytes |
| --- | --- | ---: | ---: | ---: | ---: | ---: |
| [32701649122](https://github.com/musutrade/arc-admin/actions/runs/32701649122) CI | Pinned commit, success | 1152 | 352 | 799 | 1077 | 30152 |
| [32692754787](https://github.com/musutrade/arc-admin/actions/runs/32692754787) CI | Predecessor `6563f34991a1b85cbd5590fd0ebf6a2e64532946`, success | 435 | 2 | 432 | 1048 | 29838 |
| [32701649194](https://github.com/musutrade/arc-admin/actions/runs/32701649194) CodeQL | Pinned commit, skipped | 2 | — | 0 | 0 | 0 |
| [32701649091](https://github.com/musutrade/arc-admin/actions/runs/32701649091) Supply chain security | Pinned commit, success | 365 | 207 | 157 | 140 | 490912 |

For pinned CI, scope/frontend/quality/backend occupied 41/311/285/440 runner-seconds respectively. The predecessor used the earlier topology without the scope job; its numbers are context, not a controlled before/after performance comparison. The three pinned workflow runs total 1217 runner-seconds, but their wall times overlap and must not be added as delivery latency.

`cost-summary.json` separates recorded scope steps, project/verification commands, artifact uploads, setup/teardown and residual job overhead. Classification is by the captured step names in the reproduction script; the original timestamps remain available for a different analysis. Project-command time includes embedded secret scanning, architecture audit, command startup and regex parsing; the available metadata cannot separate those costs. There is no separate Harness-Gate quality-collector or shadow-mode measurement in this before state. Wall/queue observations vary with runner availability and caches. Monetary cost is **unknown**: neither host hourly rates nor billing/allocation data were available, so self-hosting is not treated as free. No cost-saving or added-overhead claim is supported yet.

All seven CI report artifacts in these two CI samples had expired by capture (14-day retention); their metadata survives. Both security SBOM artifacts were unexpired at capture (30-day retention). This limits later content-level comparisons and is explicitly part of the baseline. Future parity work needs fresh, retained artifacts and matched source/configuration/environment before attributing any timing difference to Harness-Gate.

## Ownership and regression boundary

| Existing assurance | Owner and Harness-Gate extension boundary |
| --- | --- |
| `frontend.e2e` / Playwright browser assertions | Project-owned validation via a command hook; no Playwright built-in required |
| `backend.tests`, including `backend/tests/api_flow.rs` API/integration assertions | Project-owned validation via a command hook; generic service isolation and result parsing are reusable infrastructure |
| `frontend.fullstack-smoke`, Playwright configs and startup/migration/bootstrap script | Project-owned full-stack validation via a command hook; application topology and assertions remain in Arc-Admin |
| `workflow.api-generation`, OpenAPI export and generated client consistency | Project-owned generation validation via a command hook; no Arc-Admin generator in Generic Core |
| `workflow.production-deployment-config`, templates, release/upgrade, observability, tracing, retention and supply-chain configuration checks | Project-owned configuration/deployment validation via command hooks; these validate configuration, not permission to deploy |
| Backend/frontend/tool format, lint, compile, test and build commands; existing audit/secret policy and CI security routes | Preserve the existing project-owned blocking contract; reusable protocols may enrich evidence without moving domain policy into Core |

Structured result ingestion may reuse a machine format but does not acquire policy authority. Quality collectors supply normalized measurements; they do not turn browser/API/smoke exit statuses into CRAP or coverage, and Angular CRAP is not fabricated. Missing lossless import, service or result-format support would be a generic capability/UX gap to record, not permission to weaken one of these checks.

Task 1.3 is enforced by `project_owned_runner_replacement_preserves_generic_command_gate` in Harness-Gate's failure-path integration tests: two previously unknown project executables use the same generic configuration path, both can pass, either can block, and logs/sealed Harness-Gate evidence remain valid. This test concerns the extension boundary only; it is not Arc-Admin shadow parity or completion of OpenSpec tasks 5/6. Documentation consistency also requires the three extension layers and the no-special-case principle in Engineering Policy. No production Generic Core branch or schema change is introduced.

## Shadow observation (GH-204)

[Full shadow comparison](shadow/README.md) records all 25 Arc-Admin blockers and two prelude checks passing, both Harness-Gate variants rejected before dispatch, and two classified product gaps. Runtime parity remains blocked and merge authority remains unchanged.

## CI cost and topology (GH-206)

[Cost and ownership analysis](cost/README.md) retains the measured before CI cost, inventories duplicate commands and measurement risks, and defines a conditional single-owner target with an assurance parity argument. Added self-hosted shadow cost remains unmeasured (task 7.1 blocked); no existing gate or authority is removed.
