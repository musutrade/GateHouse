# Proposal: Optimize CI Execution Topology

## Summary

Refactor Harness-Gate's GitHub Actions execution topology to reduce repeated setup, compilation, collection, and runner cost while preserving every existing required assurance semantic.

This change inherits `docs/engineering-policy.md`. **CRAP semantics unchanged. Required assurance unchanged.** No required gate, platform, threshold, measurement series, fail-closed behavior, release authority, or `Required Quality Aggregate` check name may be weakened by this change.

## Why

The CI workflow has grown organically with the quality architecture. A normal pull request currently starts independent Linux test/build/lint jobs, full macOS and Windows tests, security audit, quality coverage/risk/critical-path collection, CLI contracts, documentation consistency, release governance contracts, and quality-script tests before the stable aggregate check can finish.

Recent hosted evidence shows cross-platform tests are already material critical-path contributors, while several jobs repeat Rust/tool installation and compilation. The next product phase will integrate generic collectors and the Rust quality core into normal project workflow. Adding that work onto the current topology first would compound cost and make later optimization riskier.

The repository therefore needs a bounded optimization phase before product workflow integration.

## Goals

- Preserve all current PR/push required outcomes and the stable `Required Quality Aggregate` contract.
- Establish a retained before/after CI timing and runner-cost baseline using hosted workflow evidence rather than local timing claims.
- Eliminate avoidable repeated installation of pinned CI tools such as `cargo-nextest`, `cargo-llvm-cov`, and `cargo-audit` where a verified/prebuilt installation or durable cache is appropriate.
- Normalize Cargo cache/target strategy so jobs do not accidentally use ineffective or inconsistent paths.
- Reduce duplicate compilation and duplicate equivalent evidence collection where reuse can preserve provenance and trust semantics.
- Keep `Required Quality Aggregate` a cheap fail-closed aggregator over child results.
- Make CI cost observable enough that future required work must account for critical-path and runner-minute impact.
- Leave the workflow ready for later `integrate-generic-quality-into-project-workflow` work without implementing that product change here.

## Non-goals

- Do not make Windows or macOS tests conditional in this change.
- Do not remove or downgrade any current PR-required job.
- Do not change CRAP `<= 30`, coverage thresholds, critical-path requirements, debt/ratchet semantics, measurement-series identity, or supported measurement boundaries.
- Do not convert required checks to advisory/optional.
- Do not change branch protection/ruleset required check names.
- Do not introduce `quality.toml`, collector orchestration from `verify`, baseline-provider UX, or preset quality integration.
- Do not add Java/Python/other ecosystem adapters.
- Do not claim a fixed percentage speedup before hosted before/after evidence exists.

## Proposed approach

1. Capture a machine-readable hosted CI topology/cost baseline for representative successful PR runs, including job wall time, setup/tool-install time, test/collection time, aggregate wait, and approximate runner-minutes.
2. Introduce version-pinned reusable CI setup primitives and consistent cache/target conventions without sharing mutable build state across trust boundaries.
3. Remove forced source installs when a pinned verified binary installation path can provide the same tool/version contract; retain explicit version evidence.
4. Reuse immutable build/evidence artifacts only where producer identity, commit/run identity, hashes, and consumer validation make reuse equivalent to current behavior.
5. Keep quality collection single-owner: downstream projection/reporting must consume retained evidence rather than recollect equivalent measurements.
6. Keep the aggregate job execution-only: it evaluates `needs` results and must not become a second collector/test runner.
7. Record hosted after-state evidence and compare assurance parity plus performance/cost deltas before acceptance.

## Acceptance

The change is acceptable only when:

- every currently required PR child outcome remains represented and fail-closed;
- macOS and Windows full tests remain PR-required;
- existing Rust risk/CRAP, coverage, critical-path, CLI-contract, docs, release-governance, security, fmt, clippy, build, and quality-script semantics remain intact;
- missing/failed/cancelled/skipped required results still make the aggregate fail according to the current event contract;
- retained before/after hosted evidence demonstrates no assurance loss and reports critical-path and runner-cost deltas;
- any artifact reuse validates immutable identity/provenance and fails closed on missing or mismatched artifacts;
- CI optimization does not rely on lowering thresholds, resetting baselines, skipping tests, or changing measurement series;
- documentation identifies which optimizations are execution-only and which future optimizations would require a separate Engineering Policy delta.

## Follow-up

After this change is accepted, begin the independent OpenSpec `integrate-generic-quality-into-project-workflow`. Risk-driven conditional cross-platform CI may be evaluated later as a separate explicit policy change with equivalence evidence; it is intentionally excluded here.