# GH-198: Final hosted reconciliation

This reconciles OpenSpec `integrate-generic-quality-into-project-workflow` tasks
8.3, 8.4 and 9.3 using the controller-confirmed GH-186/GH-187 acceptance and
GitHub metadata captured on 2026-09-10. Historical submission records remain
historical. Formal closure follows the controller's green GH-198 PR and merge;
these receipts establish the prerequisite heads, not GH-198's own CI result.

| Acceptance | GH-186 / PR #196 | GH-187 / PR #197 |
| --- | --- | --- |
| Successful CI run, attempt 1 | [34419625673](https://github.com/musutrade/Harness-Gate/actions/runs/34419625673) | [34422630969](https://github.com/musutrade/Harness-Gate/actions/runs/34422630969) |
| PR head SHA | `024f775904eb6bd50639092d3779d9e2f8dad035` | `0d2dfc23481cde02c3adc3f6659d00ca001ce3d8` |
| Tested merge SHA | `ffe3bc0e5f7dea4f864ceab653b26debb78d235d` | `a75928522eb714bc569aaef5928c8e94411107ee` |
| Collection base SHA | `9fcc319d976aed1aad4311355cece60df798ff96` | `fd0d040fad1f051c64adb18b9a644fbe3c3dd24a` |
| Required Quality Aggregate, completed success | [102695650692](https://github.com/musutrade/Harness-Gate/actions/runs/34419625673/job/102695650692) | [102704478218](https://github.com/musutrade/Harness-Gate/actions/runs/34422630969/job/102704478218) |
| Raw run/jobs and collection identity | [GH-186 input](https://github.com/musutrade/Harness-Gate/blob/d9a932b23dcfea709175285b594ca200450a9971/docs/quality/gh-187/hosted-prerequisite-input.json) | [GH-187 input](https://github.com/musutrade/Harness-Gate/blob/d9a932b23dcfea709175285b594ca200450a9971/docs/quality/gh-187/hosted-closure-input.json) |
| Normalized timing | [GH-186 timing](https://github.com/musutrade/Harness-Gate/blob/d9a932b23dcfea709175285b594ca200450a9971/docs/quality/gh-187/hosted-prerequisite-timing.json) | [GH-187 timing](https://github.com/musutrade/Harness-Gate/blob/d9a932b23dcfea709175285b594ca200450a9971/docs/quality/gh-187/hosted-closure-timing.json) |
| Configured acceptance | [receipt](https://github.com/musutrade/Harness-Gate/blob/d9a932b23dcfea709175285b594ca200450a9971/docs/quality/gh-187/hosted-prerequisite-receipt.json.gz), [summary](https://github.com/musutrade/Harness-Gate/blob/d9a932b23dcfea709175285b594ca200450a9971/docs/quality/gh-187/hosted-prerequisite-summary.json) | [receipt](https://github.com/musutrade/Harness-Gate/blob/d9a932b23dcfea709175285b594ca200450a9971/docs/quality/gh-187/hosted-closure-receipt.json.gz), [summary](https://github.com/musutrade/Harness-Gate/blob/d9a932b23dcfea709175285b594ca200450a9971/docs/quality/gh-187/hosted-closure-summary.json) |
| GitHub artifact metadata | [GH-186 artifacts](https://github.com/musutrade/Harness-Gate/blob/d9a932b23dcfea709175285b594ca200450a9971/docs/quality/gh-187/hosted-prerequisite-artifacts.json) | [GH-187 artifact](https://github.com/musutrade/Harness-Gate/blob/d9a932b23dcfea709175285b594ca200450a9971/docs/quality/gh-187/hosted-closure-artifact.json) |

Both runs completed `success` under the existing `pull_request` contract. Every
required child succeeded: Linux Test (`Test`), Windows Test, macOS Test, Quality
Coverage and Critical Paths, Security Audit, Format, Clippy, Build, Quality CLI
Contracts, Documentation Consistency, Release Governance Contracts and Quality
Script Tests. Generic Quality Shadow also succeeded and remains advisory.
The [machine-readable reconciliation](https://github.com/musutrade/Harness-Gate/blob/d9a932b23dcfea709175285b594ca200450a9971/docs/quality/gh-187/hosted-closure.json) retains every successful
job's exact ID/name/conclusion. Push-only checks retain their event-defined skips.
Both PR-head and tested-merge workflow blobs are
`502f79d8b6e9a3af43927569ca99b888c324a63c`.

## Measured cost and acceptance

The existing `ci_timing.py` normalizer and frozen topology contract are unchanged.
GH-186's existing input uses the
[hosted-after-input envelope](https://github.com/musutrade/Harness-Gate/blob/d9a932b23dcfea709175285b594ca200450a9971/docs/quality/ci-topology/hosted-after-input.json); all retained
raw jobs and run identity/timestamps were checked against GitHub again, and its
normalization reproduces byte-for-byte. GH-187 uses the same envelope.

| Measure | Retained baseline median | Accepted after-cohort median | GH-186 | GH-187 |
| --- | --- | --- | --- | --- |
| Required critical path, seconds | 903 | 878.5 | 979 | 964 |
| Total runner wall minutes | 40.6833 | 39.3333 | 39.5833 | 41.4667 |
| Linux runner wall minutes | 28.6 | 26.8750 | 29.6333 | 29.6833 |
| macOS runner wall minutes | 4.2667 | 5.0917 | 2.3667 | 4.8333 |
| Windows runner wall minutes | 5.7 | 7.3667 | 7.5833 | 6.9500 |
| Linux Test wall seconds | 222 | 224 | 215 | 253 |
| Production quality collection seconds | 600 | 836.5 | 934 | 928 |
| Configured acceptance test seconds | — | — | 20.101651443 | 20.623483927 |
| Receipt validation / upload seconds | — | — | 0 / 2 | 1 / 0 |

Baseline runs are 34303413318, 34301967575 and 34291282719; the accepted after
cohort is 34312685548 and 34311132169. Their distributions and source links remain
in the [accepted topology record](../ci-topology/after-state.md) and companion JSON.
Critical path means run creation to aggregate completion. Runner work sums active
job wall time, including advisory work, without OS billing multipliers. Per-OS
medians need not sum to the median total. Zero-second steps reflect GitHub's
timestamp precision, not zero cost. Test/validation/upload durations are already
inside job totals and must not be added again.

GH-186 has **100.5 seconds (11.4%) more required latency** and **0.25 more total
runner minutes (0.6%)** than the accepted after median. Linux work increases by
2.7583 minutes; the lower macOS occupancy does not erase that added work. GH-187
has **85.5 seconds (9.7%) more required latency** and **2.1333 more runner minutes
(5.4%)**. These are meaningful observed costs, not performance improvements.
Production quality remains the last required child (965/953 seconds job wall);
the acceptance test also runs inside that existing coverage invocation, whose
instrumented acceptance-only duration is not isolated. Both jobs' complete costs
are included. Changing source/test corpus, compiler/cache state and runner
contention prevent attributing these differences to the integration from one
run per head. No causal speedup or integration-only cost estimate is established.

The retained controller acceptance justifies the bounded acceptance work by its
12 complete direct-Rust/verify report comparisons, 40 fail-closed negatives,
identity-bound uploaded receipt and producer-reuse proof. These check orchestration
and artifact trust that collector measurements alone cannot establish. No added
production collector or per-ecosystem CI job is needed. This accepts the observed
cost with that assurance benefit; it does not hide, amortize or offset the work,
or revive ineffective compiled-cache/build-once experiments.

## Assurance parity, extension and rollback

Both hosted receipts revalidate with exact run/attempt/tested-SHA/Linux-X64
identity and the unchanged matrix: Rust, Angular/reference, mixed Angular+Rust
and unknown `nebula-unregistered-2049`, each with pass, policy violation and missing
capability cases. Forty identity/artifact/source negatives remain fail-closed.
Each producer launches once; removing its executable before reuse produces zero
additional or fallback launches. The same configuration-driven compiler,
collector protocol, evidence validation, verifier, Rust evaluator and reporting
path handles all cases. These synthetic fixtures do not broaden certification.

New ecosystem, language and framework names remain configuration/pack data.
Extension remains **collector + capability/policy pack + certification**, without
generic-core redesign or ecosystem dispatch. Unknown metric semantics still need
an explicit supported capability contract. Pack, baseline and profile evidence
remains linked from [GH-187 validation](validation.md).

Native macOS and Windows assurance stays required alongside Linux. The existing
quality-coverage job remains the sole authoritative production coverage/risk/CRAP
collector; generic projection consumes validated retained evidence. No duplicate
authoritative collection, threshold/series/policy change, CI topology change or
collector-owned decision is introduced. Required Quality Aggregate remains
unchanged and fail-closed; it checks child outcomes, including required native
jobs. See [ADR-0048](../../adr/0048-configured-ci-workflow-acceptance.md).

Rollback of this issue removes only reconciliation records/checkmarks. If a later
integration rollback is necessary, disable/remove the configured integration
layer while preserving low-level Rust evaluation, retained evidence, collector
authority boundaries, native requiredness and the aggregate contract. Never
substitute Python/collector policy decisions, relax thresholds, fabricate PASS,
or silently recollect incompatible evidence. Deferred adapters, compat
deprecation, conditional native platforms and application dogfood remain outside
this OpenSpec.

## Reproduction and local validation

Retrieve metadata with `gh api repos/musutrade/Harness-Gate/actions/runs/<run>`
and `gh api repos/musutrade/Harness-Gate/actions/runs/<run>/attempts/1/jobs?per_page=100`.
The inputs retain job labels, steps, timestamps and collection-log excerpts with
full-log hashes; each collection job's `/actions/jobs/<job>/logs` supplies the
tested `HEAD_SHA`, `BASE_SHA` and `RUN_ID`. The workflow contents API at each
PR/tested SHA supplies its blob identity. The GH-187 receipt archive digest
matches GitHub artifact 10131558936:
`sha256:34321b6ec7a9305d686da6589f88c2a2401f7e084f153b62d27c2e72ee614a14`.
Retained receipts survive hosted artifact expiry.

```bash
python3 tools/quality/ci_timing.py --input docs/quality/gh-187/hosted-prerequisite-input.json --output target/quality/gh-198/gh186-timing.json
cmp docs/quality/gh-187/hosted-prerequisite-timing.json target/quality/gh-198/gh186-timing.json
python3 tools/quality/ci_timing.py --input docs/quality/gh-187/hosted-closure-input.json --output target/quality/gh-198/gh187-timing.json
cmp docs/quality/gh-187/hosted-closure-timing.json target/quality/gh-198/gh187-timing.json
```

To revalidate each receipt, decompress it into a separate directory as
`receipt.json` and run `python3 tools/quality/ci_acceptance.py --directory <directory>`
with `GITHUB_SHA` set to its tested merge SHA, `GITHUB_RUN_ID` to its run,
`GITHUB_RUN_ATTEMPT=1`, `RUNNER_OS=Linux` and `RUNNER_ARCH=X64`.
Both validations pass with 12 cases and 40 negatives, including exact report
parity and embedded evidence hashes.

GH-198's [validation manifest](https://github.com/musutrade/Harness-Gate/blob/d9a932b23dcfea709175285b594ca200450a9971/docs/quality/gh-198/validation.json) and
[complete logs](https://github.com/musutrade/Harness-Gate/blob/d9a932b23dcfea709175285b594ca200450a9971/docs/quality/gh-198/validation-logs.tar.gz) retain actual command results.
All required checks passed: nextest (379 tests), Python unittest (346 tests),
Rust formatting, Clippy, docs consistency, strict OpenSpec validation and
`git diff --check`. Every task from 1.1 through 9.4 now has acceptance evidence.
Cargo uses `CARGO_TARGET_DIR=$PWD/target` within this workspace.
Root `harness-gate config check` and `harness-gate verify --profile ci --all` are
not applicable: no `.harness-gate/flow.toml` declares a `ci` profile. No generic
configuration was invented. No product code, workflow or generic-core changes
are part of this reconciliation.
