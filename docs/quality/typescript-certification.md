# Bounded TypeScript/Angular certification

This record covers only the retained repository reference fixture and capabilities
below, under [OpenSpec tasks 4.2–4.3](../../openspec/changes/typescript-angular-reference-adapter/tasks.md)
and [ADR-0040](../adr/0040-language-agnostic-evidence-policy.md).
[GH-136 final review](gh-136/README.md) links merged predecessor PRs and successful
required CI through task 4.2, retained real-tool evidence and repeated local replay.
Task 4.3 review is recorded; final acceptance/closure requires successful required
CI on the review PR and controller merge.
This is a bounded TypeScript/Angular reference adapter, not universal frontend
certification. This record confers no release authority or general Angular/platform certification.

## Exact fixture and toolchain

The fixture is `tools/quality/fixtures/typescript-angular`, including its live
Rust quote provider. [GH-134's window](https://github.com/musutrade/Harness-Gate/blob/d9a932b23dcfea709175285b594ca200450a9971/docs/quality/gh-134/window.json) and Git bundles retain
these complete, clean fixture histories (distinct from delivery commits):

| Measurement pair | Base revision | Head revision | Target / run IDs |
| --- | --- | --- | --- |
| Compatible | `c549ccc9994d63c28fc979a1d4d34203816adbd6` | `e8c569b30461a1502eb7d35336672799157465d3` | `node-jsdom`; `compatible-base`, `compatible-head` |
| Regression | `e09b480256c2bc9ca30d4d1e5f70dea5b312eae4` | `6322986c399e84b161e947bda5c72c983b78c7bf` | `node-jsdom`; `regression-base`, `regression-head` |

The independent [contract fixtures](https://github.com/musutrade/Harness-Gate/blob/d9a932b23dcfea709175285b594ca200450a9971/docs/quality/gh-133/index.json) retain native revision
`9dbb950e57f0985976cc0f5db04c47d6cb401d23`, target `native-contract`, runs
`gh133-compatible` and `gh133-breaking`; their archives pin the actual scenario
source bytes, generated clients and caller receipts. They are not a coverage
base/head history or interchangeable coverage baselines.

| Tool / environment | Exact measured value |
| --- | --- |
| Angular / CLI / application and unit-test builders | `22.0.8` |
| Node / npm / lockfile format | `24.18.0` / `11.16.0` / `3` |
| TypeScript | `6.0.2` |
| Vitest / Istanbul provider | `4.0.8` / `@vitest/coverage-istanbul@4.0.8` |
| DOM | `jsdom@28.0.0` |
| Client generator / OpenAPI compatibility | `openapi-typescript-codegen@0.29.0` / `oasdiff 1.11.7` |
| Rust / Cargo / provider coverage | `1.97.1` / `1.97.1` / `cargo-llvm-cov 0.9.0` |
| Native collection Python | `3.14.4` |
| Native collection platform | `Linux-7.0.0-31-generic-x86_64-with-glibc2.43` |
| Execution | Node jsdom tests against loopback Rust provider; production browser build; `CI=true`, `NG_CLI_ANALYTICS=false` |
| Advisory replay environment | Git, Python `3.12`, GitHub `ubuntu-24.04`; actual interpreter/platform recorded in every summary |

The [frozen toolchain](../../tools/quality/fixtures/typescript-angular/toolchain.json)
describes the original native-only milestone; its exclusions are not superseded
by claims beyond this matrix. Raw manifests retain commands, runtime versions,
configuration digests, installed inventory, sources and source maps. The npm lock
SHA-256 is `f2eed8594891c8350ed90ad3613d2d17e5fd5c24c8c6fe9ceb3a72b9e955c93c`.
Replay does not recollect frontend measurements or certify the hosted runner as
a new native measurement platform.

## Series and support matrix

Coverage uses collector `typescript-reference@1`, identity
`typescript-original-source/v1@1`, normalization
`istanbul-start-line-max-innermost/v1@1`, runtime `24.18.0/jsdom@28.0.0`, target
`node-jsdom`. Its exact series ID is
`measurement-series/v1:b680df6dc529cafbbadf4ebba23627d2f34c6c16d30d22dcf5210b29c727a408`.
The full descriptor, including tool/configuration fingerprint and requested
metric set, is retained in [normalized evidence](https://github.com/musutrade/Harness-Gate/blob/d9a932b23dcfea709175285b594ca200450a9971/docs/quality/gh-134/compatible-head/normalized.json)
and the [advisory summary](https://github.com/musutrade/Harness-Gate/blob/d9a932b23dcfea709175285b594ca200450a9971/docs/quality/gh-135/summary.json). A series listing a metric does
not imply support: each record's capability state determines availability.
Contract/provider/consumer series descriptors and exact IDs are separately
published in that summary under `checks.contracts.*.series`.
Changed tools, configuration, normalization, identity or boundaries require
explicit baseline migration; Rust and frontend series cannot share baselines.

| Capability / scope | Bounded disposition | Reproducer and evidence |
| --- | --- | --- |
| Original TypeScript file/function/method line and function coverage | Supported: exact integer statement-start line/function-hit counters for the five source paths in the acceptance window; zero denominators are `not_applicable`, with no value | [88 native comparisons, two pairs, threshold and debt reports](https://github.com/musutrade/Harness-Gate/blob/d9a932b23dcfea709175285b594ca200450a9971/docs/quality/gh-134/results.json); [acceptance tests](../../tools/quality/tests/test_typescript_acceptance.py) |
| Source identity and map validation | Supported for uniquely mapped original TypeScript declarations and digested transformation inputs; ambiguity/staleness fails closed | [Identity evidence](gh-130/README.md); [semantic reproducers](../../tools/quality/tests/test_typescript_semantics.py) |
| Generic thresholds, regression, legacy/new/regressed debt | Supported for the retained line-coverage policies and compatible series; compatible pass and deliberate regression fail | [Policy evidence](gh-132/README.md); [real window](gh-134/README.md) |
| OpenAPI breaking-change count, compatibility and generated-client drift | Supported only for retained quote-contract scenarios; breaking interface blocks despite green local components; changed inputs/equal output bytes fail closed | [Contract results and limitations](gh-133/README.md); [contract reproducers](../../tools/quality/tests/test_typescript_contracts.py) |
| TypeScript branch/region coverage; template/generated multi-source coverage | `unsupported`; native counters or production maps do not establish normalized support | [TS-02 semantics](typescript-source-semantics.md#ts-02-transformation-provenance-and-boundaries); [negative window](gh-134/README.md#negative-matrix) |
| Route execution, real browser E2E, SSR | Unavailable; route metrics remain `unsupported`; production build/jsdom is not execution evidence for these capabilities | [Collector limits](typescript-collector.md); [native scope](../../tools/quality/fixtures/typescript-angular/evidence/README.md) |
| Complexity, CRAP, mutation, security, accessibility, bundle size and performance | `unsupported`; no numeric defaults or support claims | [Complete request and rejection tests](../../tools/quality/tests/test_typescript_reference.py); [negative window](gh-134/README.md#negative-matrix) |

## Architectural mismatch dispositions

The [design register](../../openspec/changes/typescript-angular-reference-adapter/design.md#architectural-mismatch-register)
remains authoritative. Every disposition below links both executable reproduction
and retained evidence; none introduces a generic schema or policy amendment.

| ID | Disposition and reproducer | Evidence |
| --- | --- | --- |
| TS-01 | Full original-source identity; ambiguous/reduced identity unavailable. `test_ambiguous_and_incomplete_parser_joins_rejected` in [semantic tests](../../tools/quality/tests/test_typescript_semantics.py) | [GH-130](gh-130/README.md), [fresh negatives](gh-134/README.md#negative-matrix) |
| TS-02 | One original source per subject; transformation inputs are artifacts; multi-source coverage unsupported. `test_template_and_generated_multisource_remain_unsupported` and source-map negatives in [semantic tests](../../tools/quality/tests/test_typescript_semantics.py) | [GH-130](gh-130/README.md), [GH-134](gh-134/README.md#negative-matrix) |
| TS-03 | Every requested metric has an explicit state on each record; omission and per-kind requests rejected. [Collector reproducers](../../tools/quality/tests/test_typescript_reference.py) also run against fresh GH-134 bytes | [GH-131](gh-131/README.md), [GH-134](gh-134/README.md#negative-matrix) |
| TS-04 | Equal generated bytes with changed input digests cannot silently satisfy freshness; fail closed, narrowed fixture support. [Contract reproducers](../../tools/quality/tests/test_typescript_contracts.py) | [GH-133 disposition](gh-133/README.md#fail-closed-behavior-and-bounded-capability) |

## Opt-in operation and rollback

Manually dispatch [Frontend Advisory](../../.github/workflows/frontend-advisory.yml)
on the desired ref, or run:

```bash
python3 tools/quality/typescript_advisory.py --output target/quality/frontend-advisory
```

Use a new output directory. The job replays the four retained collections through
the accepted collector/evidence/policy flow, replays both contract scenarios and
runs acceptance/contract rejection suites. Intentional negative outcomes must be
failures inside their reports for the advisory run to pass. No Node install,
Angular build, coverage collection, provider launch or baseline refresh occurs.
The job uploads raw replay inputs, normalized evidence, policy/project reports,
test logs, summary and SHA-256 artifact inventory even after failure, for 30 days.
Committed native archives and accepted baselines retain durable evidence beyond
hosted artifact expiry. The summary identifies the replayed checkout; it does
not grant required-check authority.

Rollback disables the `Frontend Advisory` workflow in GitHub Actions, or removes
its manual trigger/job. For local opt-in projects, deselect `typescript-reference`
from their collector invocation/manifest. Neither operation deletes retained
evidence, debt history or accepted baselines. This change leaves `ci.yml`,
`Required Quality Aggregate`, all required dependencies, its check identity,
Rust adapter defaults and release workflows unchanged. Required hosted CI must
pass on the submitted SHA before merge; the controller owns that check and merge.
