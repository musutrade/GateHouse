# CI topology and hosted pre-change baseline (GH-164)

Scope: OpenSpec `optimize-ci-execution-topology` tasks 1.1–1.3 only. Engineering Policy is normative. CRAP semantics unchanged; required assurance unchanged. Tasks 2–7 remain unimplemented.

## Frozen assurance contract

Source commit: `f11467e133cc1c791dabcb21e8af8e81e09a1950`. Pre-change `.github/workflows/ci.yml` Git blob: `dfcf4b07f4e9f73762622fda4cec939bb8a3c5e2`. The [machine-readable contract](../../../tools/quality/fixtures/ci-topology.json) records every job, displayed check name, runner matrix, event, dependency and outcome. The workflow triggers on PRs targeting `main` and pushes to `main`, with read-only contents permissions. PR concurrency cancellation is retained; cancellation is never success.

**Full macOS and Windows tests remain PR-required.** Both matrix instances belong to `test-cross-platform`; `fail-fast: false` and the full locked nextest invocation with `--no-fail-fast` are retained. A failed matrix result blocks the aggregate.

Inspection exposed an existing implementation mismatch: native tests already ran on PRs, but `ci_quality.PUSH_ONLY` still listed `test-cross-platform`. Reproducing the original classification with a failed native result returned no failures. GH-164 moves only that child into `COMMON` so the aggregate enforces the requirement stated by this OpenSpec. This is an assurance repair, with no scheduling, install, cache, command, threshold or measurement change. The fixture records the historical gap instead of presenting it as valid policy.

| Job ID | Stable displayed check name(s) | PR | Push | Required semantic outcome |
| --- | --- | --- | --- | --- |
| `test` | `Test` | Required | Required | Full locked Linux nextest suite; exported schema must match committed schema. |
| `test-cross-platform` | `Test (macos-latest)`; `Test (windows-latest)` | Required | Required | Full locked native macOS and Windows nextest suites with --no-fail-fast; both PR-required. |
| `security-audit` | `Security Audit` | Required | Required | cargo audit --deny warnings; installation errors block. |
| `fmt` | `Format` | Required | Required | cargo fmt --check. |
| `clippy` | `Clippy` | Required | Required | Locked all-targets/all-features Clippy with warnings denied. |
| `build` | `Build` | Required | Required | Locked Linux release build; artifact absence retains existing ignore policy. |
| `build-cross-platform` | `Build (macos-latest)`; `Build (windows-latest)` | Skipped | Required | Locked native macOS and Windows release builds. |
| `coverage` | `Code Coverage` | Skipped | Required | Full tarpaulin LLVM coverage; Codecov upload retains fail_ci_if_error: false. |
| `quality-coverage` | `Quality Coverage and Critical Paths` | Required | Required | Fresh legacy coverage, production coverage, base/head exact risk and isolated critical-path evidence; all four stages required; always retain available candidate evidence. |
| `quality-generic-shadow` | `Generic Quality Shadow` | Advisory | Advisory | Advisory projection of retained candidate with identity/hash validation; no aggregate dependency or release authority. |
| `quality-contracts` | `Quality CLI Contracts` | Required | Required | Linux CLI contract snapshots, exit codes and output evidence. |
| `quality-contracts-cross-platform` | `Quality CLI Contracts (macos-latest)`; `Quality CLI Contracts (windows-latest)` | Skipped | Required | Native macOS/Windows structured CLI contracts. |
| `quality-baseline` | `Quality Performance Baseline (ubuntu-latest)`; `Quality Performance Baseline (macos-latest)`; `Quality Performance Baseline (windows-latest)` | Skipped | Required | Native Linux/macOS/Windows benchmark and size candidates, five samples, existing performance policy. |
| `docs-consistency` | `Documentation Consistency` | Required | Required | Documentation/examples/presets/migration/schema/policy-anchor/link/sandbox consistency. |
| `release-contracts` | `Release Governance Contracts` | Required | Required | Release governance unit tests and compile checks; publication authority unchanged. |
| `quality-scripts` | `Quality Script Tests` | Required | Required | All quality Python unit tests and compile checks. |
| `quality-required` | `Required Quality Aggregate` | Aggregate | Aggregate | Always evaluate event-applicable needs; only exact success passes. Stable ruleset check name; no collection or product compilation. |

The explicit matrix job name `Quality Performance Baseline` receives GitHub’s OS suffix in each displayed check, confirmed from [successful pre-change push run 34292369598](https://github.com/musutrade/Harness-Gate/actions/runs/34292369598).

`Required Quality Aggregate` remains the exact ruleset-facing check name, with `always()` and all 15 existing dependencies. Its complete dependency list is the machine contract’s `quality-required.needs`; `Generic Quality Shadow` is excluded. Every event-required dependency must return exactly `success`. Missing children/results and failure/cancellation/skips fail closed. Only build-cross-platform, coverage, quality-contracts-cross-platform and quality-baseline are push-only; their absence/skips on PRs do not waive another child. Existing PR handling ignores these non-required results. The aggregate only checks out source, sets up Python and evaluates `EVENT_NAME` / `toJSON(needs)`.

### Measurement and authority outcomes retained

- Legacy six-module coverage remains at least 80% per module and aggregate. Production executable-line coverage remains at least 80% for each blocking boundary and the count-weighted aggregate; informational adapters remain informational. Production denominator/exclusion and current source inventory are unchanged.
- Changed production functions require exact rational CRAP <= 30. Selected functions and changed functions with cyclomatic complexity > 10 additionally require line and region coverage independently >= 80%. Historical debt remains visible, new debt is forbidden, and existing debt and accepted improvements cannot regress or reset lineage. Branch coverage stays unsupported; unsupported ecosystems cannot invent CRAP.
- The currently certified source/series boundary in `source_measure.py`, `function_risk.py`, and `production-source.json` remains authoritative. Uncertified changes fail for measurement review. Historical ADR source counts are not an instruction to undo subsequently accepted boundary extensions.
- Critical paths retain all mandatory IDs/platform applicability in `critical_paths_policy.json`, isolated source-region and observable evidence, and >= 95% applicable-row coverage. Missing required evidence fails.
- Candidate collection retains the four required stages (legacy, production, risk, matrix), exact base/tested-head/run/attempt identity, full history and independent snapshots, tool/series identity, raw LLVM evidence and artifact digests. Failed or incomplete stages and stale/mixed/missing/modified artifacts cannot verify. Available diagnostics upload with `always()`; upload policies remain exactly as in the workflow.
- Rust retains generic/release decision authority. Python collectors and advisory projections cannot grant final delivery authority. Baseline candidates require independent review; this timing baseline accepts no product quality debt or measurement baseline. Release governance and publication permissions remain unchanged.
- Push-only benchmarks retain uninstrumented native measurement, five samples and the existing 15% regression policy. Codecov transport remains non-blocking, as already configured; the tarpaulin collection command remains required on pushes.

Policy and decision records: [Engineering Policy](../../engineering-policy.md), [ADR-0039](../../adr/0039-required-risk-and-traceability-gates.md), [ADR-0040](../../adr/0040-language-agnostic-evidence-policy.md), and the [OpenSpec design](../../../openspec/changes/optimize-ci-execution-topology/design.md).

## Hosted pre-change evidence

The sample is the three most recent successful `pull_request` CI runs returned on 2026-09-09 before implementation: PRs #163, #162 and #161, each attempt 1. These are successful documentation and test-repair PRs using the identical workflow blob above. They are a small observational sample, not a cold/warm-cache experiment or a guaranteed performance budget. Failed/cancelled runs and earlier workflow versions are excluded.

[Retained API fields and collection-log excerpts](https://github.com/musutrade/Harness-Gate/blob/d9a932b23dcfea709175285b594ca200450a9971/docs/quality/ci-topology/hosted-input.json) bind repository, workflow ID/path, run/attempt, PR URL, branch head, event, timestamps, conclusions, runner labels, job URLs and every step timestamp. The run API returned an empty `pull_requests` array; PR numbers were resolved by branch. Exact event base and tested merge SHAs come from `BASE_SHA`, `HEAD_SHA`, and `RUN_ID` in each collection job log, not from today’s PR base or an inferred parent. Log URLs, excerpts and full downloaded-log SHA-256s are retained. No local timing is used.

[Normalized timing evidence](https://github.com/musutrade/Harness-Gate/blob/d9a932b23dcfea709175285b594ca200450a9971/docs/quality/ci-topology/hosted-baseline.json) retains input SHA-256, identities, per-job/per-step wall time, categories, critical path and runner minutes. Reproduce it with:

```bash
python3 tools/quality/ci_timing.py --input docs/quality/ci-topology/hosted-input.json --output target/quality/ci-topology-baseline.json
```

| Hosted run | To aggregate (s) | Workflow wall (s) | Setup / tool installs (s) | Quality collection (s) | Aggregate (s) | Runner min Linux / macOS / Windows |
| --- | ---: | ---: | ---: | ---: | ---: | --- |
| [Run 34303413318](https://github.com/musutrade/Harness-Gate/actions/runs/34303413318) | 1141 | 1191 | 197 / 453 | 821 | 7 | 33.30 / 4.27 / 5.65 |
| [Run 34301967575](https://github.com/musutrade/Harness-Gate/actions/runs/34301967575) | 903 | 919 | 183 / 423 | 600 | 6 | 28.60 / 3.25 / 5.70 |
| [Run 34291282719](https://github.com/musutrade/Harness-Gate/actions/runs/34291282719) | 846 | 864 | 195 / 392 | 560 | 9 | 28.25 / 4.93 / 7.50 |

Time to required aggregate has median **903 s**, range **846–1141 s**. `Quality Coverage and Critical Paths` is the last required child in all three runs. Aggregate dispatch after that child takes 2–3 s; aggregate execution takes 6–9 s. Advisory shadow work can extend workflow completion after the aggregate. These results supersede the proposal’s earlier illustrative native-test timings for this sample, and support no optimization claim.

### Normalization v1 and limits

- All durations use hosted UTC timestamps at API second precision. Job/step wall time is completed minus started. Skipped jobs and steps contribute zero runner time; skipped jobs have no attributed OS. OS comes from hosted runner labels. Unknown/ambiguous OS, mixed attempts, missing required matrix instances and unsuccessful required jobs reject the sample.
- Workflow wall is `run_started_at` to the latest active job completion. API lifecycle (`created_at` to `updated_at`) is recorded separately because `updated_at` is not a precise execution end. Developer critical path is run creation to aggregate completion, including queue/dispatch. This is not a sum of parallel jobs. Aggregate wait is creation to aggregate start; dispatch gap is last required child completion to aggregate start. All required pre-aggregate jobs currently fan out independently.
- Categories are disjoint. Setup covers runner setup, checkout, Rust/Python setup and cache steps. Tool installation covers named cargo tool/tarpaulin installers. Quality collection is the single named fresh evidence collection step, including its internal compilation, tests and measurement. Execution contains other tests/builds/checks and generic projection. Artifact time covers named upload/download steps; post steps are cleanup. Any job time outside named steps is retained as unattributed runner overhead.
- Step granularity cannot separate compilation from tests inside nextest/collection, internal package download from installation, or cache-hit effects. No internal durations are invented. Setup and install totals sum concurrent work and must not be added to developer critical path. Tool-install observations include unpinned existing installers; this issue does not normalize tool versions.
- Runner wall minutes sum active job walls / 60 separately for Linux, macOS and Windows, including the aggregate, advisory shadow and job overhead, excluding queue wait. They are approximate runner consumption, not billed minutes or currency: no OS billing multipliers, minimum-minute rounding, account rates or free allowances are assumed.
- Later after-state samples must use this same normalization and retain source/workflow identities. Any changed step names require reviewed category mapping; no before/after percentage improvement is claimed here.

## Validation

See the [GH-164 validation record](validation.md) for the reproduced aggregate gap, regression fixtures, exact commands/results and environment limitations. Task 1 completion is independent of the later optimization and hosted after-state acceptance tasks.
