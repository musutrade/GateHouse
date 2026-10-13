# Arc-Admin observation and shadow parity (GH-204)

Arc-Admin's full validation passed all **25 configured blockers and two prelude checks**. Harness-Gate rejected the unchanged execution import, with and without the GH-203 quality configuration, before component selection or dispatch. Runtime parity is **blocked**, with **zero unexplained discrepancies**. These observations complete tasks 5.1–5.4; they do not accept the whole OpenSpec change or transfer merge authority.

The [complete matrix](matrix.md) includes the 23 explicitly required steps and both additional steps that still block when selected. Its [machine form](matrix.json) is reproduced from [hashed observations](observation.json), the original inventory and retained native reports. Harness-Gate `ERROR`/exit 1 describes validation evidence. `NOT_RUN` never means PASS, a failed project test, or a workflow/lifecycle state. Existing Arc-Admin `cargo flow`, hooks and CI retain all authority.

## Source and execution

The source was downloaded into this workspace's disposable `target/quality/gh-204/source`, never read from another checkout. Arc-Admin main was confirmed as `9982ed556eaf997910824d7b682946147c81a16a`, the same revision frozen in GH-201. Both engines used that tracked tree; `git diff --exit-code HEAD` was clean before and after. The exact GH-202 import was staged in `.shadow-execution/flow.toml` and `.harness-gate/flow.toml`; only the latter has the exact GH-203 quality configuration/packs beside it. No original step, dependency, service, threshold or blocker was edited.

Arc-Admin ran `cargo flow scope --all` and `cargo flow verify --profile full --all` (both exit 0). Harness-Gate's built binary ran `--project-root <snapshot> --config <variant>/flow.toml scope --all` and `verify --profile full --all` (all exit 1). The [command receipts](evidence/harness-commands.json) retain both variants. Post-Arc Harness-Gate runs left every native report hash unchanged; those pre-existing PASS reports belong exclusively to Arc-Admin.

Database/config override variables were explicitly removed; the npm cache and Cargo target were workspace-local. Initial `npm ci --prefix frontend` failed with `ENOENT` creating `/home/gem/.npm/_cacache`; rerunning with a workspace cache succeeded. Exact error, command and impact are retained in the observation. No test dependency remained missing for the completed Arc run. [Tool versions](evidence/tool-versions.json) identify the execution environment; timings are raw evidence, not task 7 cost attribution.

## Discrepancy classification

Each discrepancy has exactly one of the four allowed categories:

| ID | Category | Observation and follow-up boundary |
| --- | --- | --- |
| HG-CAP-001 | Harness-Gate capability gap | Arc-Admin runs shared-service consumers sequentially. The import retains their empty dependencies; Harness-Gate rejects unordered use of `test-postgres` with `HGCFG-SHARED-SERVICE`. Preserve blockers and reconcile execution-order semantics in task 8 before rerunning parity. |
| HG-UX-001 | Harness-Gate UX gap | Structural import and quality-binding validation succeeded without exercising complete production loading. Migration readiness needs a full preflight distinct from those narrower checks. The runtime error itself correctly identifies both conflicting users. |

No Arc-Admin issue or expected stricter generic-quality difference was observed. Generic quality never ran: absent trusted collector/baseline provisioning from GH-203 is still a prerequisite, not a measured quality failure or PASS. Task 8 remediation and task 6 controlled negative scenarios remain separate work. No dependency was added just to bypass the rejection.

## Services, isolation and artifacts

Arc-Admin created `postgres:16-alpine` on a dynamic loopback-only port. Retained inspection, `pg_isready` and TCP receipts show availability; backend/API tests and real Angular–Axum–PostgreSQL smoke passed. The specific owned container was absent after verification. The frozen service manager caches that service during sequential execution. This is not evidence of Harness-Gate service startup, reuse or cleanup: its loader stopped first.

The native workflow tests passed inherited-variable removal, process-session isolation and database safety cases; both database-consuming steps retain `DATABASE_URL` removal and service injection of `TEST_DATABASE_URL`. These are bounded observations, not an adversarial environment experiment. The matrix explicitly records Harness-Gate isolation as unobserved.

All 25 native command logs, scope, secret/audit reports and the final JSON/Markdown verification reports are retained under `evidence/arc/`. Native parsers report result counts in `test_result.json`. Intentional panic/FAIL strings inside successful negative unit tests are not gate failures. Generated Playwright/build outputs have a [path/size/hash inventory](evidence/generated-artifacts.json); large generated outputs are reproducible locally and are not bundled. Absolute source/workspace prefixes are replaced with placeholders in retained text; original-byte and normalized-byte SHA-256 values are recorded. No result content is removed. Harness-Gate produced diagnostics, no new verification or quality report, and no command artifacts.

## Reproduction and validation

Verify retained evidence offline and reproduce the matrix byte-for-byte:

```bash
python3 docs/dogfood/arc-admin/shadow/reproduce.py
```

Repeat live observations in a **new** directory inside the current workspace (authenticated `gh`, Git, Docker, the pinned Rust/Node toolchains, npm and Playwright runtime dependencies are required):

```bash
CARGO_TARGET_DIR="$PWD/target" cargo build --manifest-path tools/harness-gate/Cargo.toml --locked
python3 docs/dogfood/arc-admin/shadow/run.py --output target/quality/gh-204-repeat
```

The runner fetches the pinned commit, records current main separately, copies the unchanged overlays, installs npm dependencies with a local cache, executes both engines and records exit codes and report changes. It never creates a `ci` profile. If main has advanced, this repeats the pinned observation; refresh the baseline separately before calling it current. Inspect a new run before updating classifications or retained evidence. The pinned run's service inspection receipts were captured during execution; repeat those probes for the new run's owned container if investigating service behavior.

[Local validation evidence](validation.json) records actual commands, failures/retries and limitations. Root `harness-gate config check` and `harness-gate verify --profile ci --all` are **not applicable** because this source repository has no project-local flow or `ci` profile. Hosted Required Quality Aggregate is **CI pending**, never inferred green from local tests. Existing CI topology is unchanged.

Related records: [OpenSpec design](../../../../openspec/changes/dogfood-harness-gate-on-arc-admin/design.md), [tasks](../../../../openspec/changes/dogfood-harness-gate-on-arc-admin/tasks.md), [ADR-0040](../../../adr/0040-language-agnostic-evidence-policy.md), [ADR-0044](../../../adr/0044-trusted-quality-baselines.md), [quality prerequisites](../quality/README.md).
