# Rust migration compatibility inventory (OpenSpec 0.3 / GH-111)

Current runtime ownership and Python freeze/retirement rules are defined in the
[Python retention policy](python-retention.md). The released Rust core owns final
generic decisions; Python interfaces below remain adapter/reference tooling.


Review baseline: `ad54d8df6d21d3f6e3a0b5ee83918ae078a84d61`.
All rows below are **frozen machine or consumer contracts**. The current Rust
path remains the release authority until a separately reviewed equivalence
acceptance and rollout. GH-111 introduces no projection, generic evaluator,
required-check replacement, baseline acceptance or language adapter.

| Surface / authoritative implementation | Compatibility requirement | Existing validation oracle |
| --- | --- | --- |
| `ci_quality.py collect` | Fresh output directory; same explicit base/head/run; four stages `legacy`, `production`, `risk`, `matrix`; exact commands/exits and timings retained; errors/incomplete stages block. Never silently collect against another base. | `test_ci_quality.py`, retained GH-96 candidate |
| `ci_quality.py verify` and candidate schema **integer 1** | `candidate: true`, commit/base/run equality, exact stage set, every status `success`, required artifact set and every SHA-256 checked within candidate root. Review-only candidate is never an accepted baseline. | Candidate identity/stage/artifact negative tests in `test_ci_quality.py` |
| Original coverage, `coverage.py` | Existing six-module and aggregate executable-line threshold remains 80%; preserve its original denominator and reports. | `test_quality_common.py`, GH-96 `coverage.json` |
| Production coverage, `production_coverage.py`, `production-source.json` | All ten required boundaries and count-weighted aggregate require 80% executable lines; preserve raw line/function/region counts, source ownership, exclusion reasons and test exclusion behavior. Real container adapter remains informational. | `test_production_coverage.py`, GH-96 `production.json` |
| GH-94 Rust risk, `source_measure.py` | Keep analyzer `harness-gate-rust-measure/0.2.0`, rule `mccabe-rust-2/1`, instrumentation `closure-black-box/1`, mapping `insertions-utf8/1`, selection `gh94-and-staged-snapshot/1`. Six supported files, closures, 32 selected responsibilities; original counts, qualified identity and exact rational `crap_line` retained. | `test_source_measure.py`, `test_function_risk.py`, `test_risk_bundle.py`, GH-96 base/head risk archives |
| Rust risk policy and limits | Changed functions require exact `crap_line <= 30`; selected functions and changed functions with CC >10 independently require line and region coverage >=80%. Unmodified historical debt stays explicit. Branch is unsupported. Changed Rust sources outside supported files require measurement review; no repository-wide CRAP claim. | Same-series comparison and negative fixtures above; ADR-0039 |
| Isolated critical-path matrix, `critical_paths.py`, `critical_paths_collect.py`, `critical_paths.toml`, `critical_paths_policy.json` | Preserve `critical-path-source-v2`, six mandatory paths, >=95% applicable-row completeness, per-row isolated test/source-region/observable evidence, platform applicability, source bindings and bundle integrity. Aggregate test-suite success is not isolated evidence. | `test_critical_paths.py`, GH-96 `critical-paths.json` and isolated runs |
| `quality-evidence.schema.json` and `quality_evidence.py` | Keep Rust complexity schema version **string "1"**, canonical series/symbol keys, raw counters, source bytes/digests, qualified symbols and spans. It must not be broadened in place into `harness-evidence/v1`. | `test_quality_evidence.py`, `test_complexity_analyzer.py` |
| Artifact digests and source integrity | Candidate paths must remain confined; missing/modified artifacts block. Base/head archives and original manifest bytes match Git; raw LLVM/manifest digests match reports; checked-out product sources match commit. Preserve raw JSON, LCOV, Cobertura, source archives, logs and matrix bundles. No projection may replace the originals or invent source integrity. | `test_ci_quality.py`, `test_source_measure.py`, `test_critical_paths.py` |
| CI job `quality-coverage`, display name `Quality Coverage and Critical Paths` | Required on both PR and push; full history, event-specific base/head/run, `collect`, unconditional available-evidence upload. Failed/killed collection never becomes success. | Workflow contract test in `test_ci_quality.py` |
| `ci_quality.py aggregate`, CI job `quality-required`, display name `Required Quality Aggregate` | Stable required consumer identity, `always()` execution and unchanged dependency semantics below. Missing, failed, cancelled, skipped or unknown required results fail closed. | `test_ci_quality.py`, `tools/release/tests/test_release_policy.py` |

## Stable aggregate consumers

Both PR and push require `test`, `security-audit`, `fmt`, `clippy`, `build`,
`quality-coverage`, `quality-contracts`, `docs-consistency`, `release-contracts`,
and `quality-scripts`. Push additionally requires `test-cross-platform`,
`build-cross-platform`, `coverage`, `quality-contracts-cross-platform`, and
`quality-baseline`; only those additional jobs may skip on PRs. Unsupported
events are rejected. The workflow retains all dependencies and passes their
actual results through `NEEDS_JSON`.

The main required-check ruleset consumes the stable display name, as recorded
in the workflow and [ADR-0039](../adr/0039-required-risk-and-traceability-gates.md).
The release eligibility consumer
[`tools/release/release_policy.py`](../../tools/release/release_policy.py)
also requires `Required Quality Aggregate` from the expected CI workflow for the
protected source commit. No ruleset or consumer is changed by this issue.

## Evidence and migration boundary

Implementation links: [collector](../../tools/quality/ci_quality.py),
[workflow](../../.github/workflows/ci.yml),
[old evidence schema](../../tools/quality/schema/quality-evidence.schema.json).
Historical oracle: [GH-96 validation and retained artifacts](gh-96-validation.md).
Current validation: [GH-111 evidence](gh-111-validation.md).

The [generic model](project-model.md) is a separate schema. Future tasks 2–10
must project the same retained candidate and raw evidence, preserve Rust series
and capability limits, and treat any outcome/debt/provenance mismatch as a
blocking compatibility failure. Neither this inventory nor local validation
constitutes Rust equivalence acceptance or baseline approval. ADR-0039 remains
the accepted authority; the full architecture ADR belongs to task 10.4.

GH-116 adds the [Rust reference adapter](rust-reference-adapter.md) and
[historical compatibility validation](gh-116-validation.md) for tasks 7.1–7.5.
The current Rust required gates and baseline approval rules remain authoritative.
