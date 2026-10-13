# ADR-0039: Require coverage, risk and traceability evidence in CI

- Date: 2026-09-07
- Status: Proposed; implementation submitted independently of baseline acceptance
- Scope: GH-96; OpenSpec `strict-json-results-and-risk-based-quality-gates`, tasks 8.1–8.4
- Evolves: [ADR-0025](0025-phase-1-quality-baseline-gates.md)

## Context

The Phase 1 workflow collected coverage only on pushes to main. GH-91 added
production boundaries, GH-92 added risk contracts, GH-94 established the tested
AST/closure measurement series, and GH-95 bound critical paths to isolated source
evidence. These tools need a required PR execution path. Historical measurements
and the accepted Phase 1 baseline keep their original meaning and files.

## Decision

`Quality Coverage and Critical Paths` runs on both PRs and pushes. A single local
and CI entry point, `ci_quality.py collect`, requires four stages: the original
six-module coverage gate, expanded production coverage, base/head function risk,
and the isolated critical-path matrix. The production gate reuses the original
run's LLVM JSON/LCOV, preserving both independent denominators without rerunning
tests. Failed stages remain failed while later stages collect diagnostic evidence.

`Required Quality Aggregate` keeps its existing check name and requires this job
on both events. Every applicable job must return exactly `success`; missing,
failed, cancelled and skipped results fail. Cross-platform tests/builds/contracts,
tarpaulin and performance candidates retain their explicit push-only schedule.
Their intentional absence on PRs is not a waiver for any PR-required job.
No repository ruleset, branch protection or publication permission changes.

Current-contract clarification (GH-164): the later CI workflow runs full macOS
and Windows tests on PRs as well as pushes. The
[frozen topology record](../quality/ci-topology/baseline.md) retains that requirement
and repairs the stale push-only aggregate classification for native tests.
Cross-platform builds/contracts, tarpaulin and performance remain push-only.
This clarification does not alter measurement or release authority.

### Thresholds and measurement limits

- The original six-module gate still requires 80% per module and aggregate.
- Ten production boundaries, including app/project/doctor/service-core, each
  require 80% executable lines, as does their count-weighted aggregate. The real
  container CLI adapter remains informational. Test exclusions cannot inflate
  the production denominator.
- Risk uses the unchanged GH-94 series: changed functions require exact
  `crap_line <= 30`; selected functions and changed functions with CC >10 also
  require line and region coverage independently >=80%. Historical unmodified
  debt remains explicit. Branch coverage remains unsupported.
- That series measures six files, including their closures and all 32 selected
  responsibilities. A changed Rust source outside those files fails with a
  measurement-review diagnostic. This conservative integration cannot certify
  repository-wide risk. Expanding supported sources requires tested analyzer
  support and an independent same-series candidate; it cannot silently pass
  through unmeasured changes. Test-only edits in other source files also require
  that review under this conservative file boundary.
- All six independently listed mandatory paths must pass, and at least 95% of
  applicable matrix rows must have isolated test, source-region and observable
  evidence. This supplements risk with the existing behavioral assertions.

### Provenance, failures and retention

CI checks out the tested commit with full Git history. PRs compare against the
event's base SHA; pushes compare against `before`. Missing base objects or a zero
SHA are errors, never a request to invent a baseline. The collector refuses an
existing output directory, verifies checked-out product sources against Git,
archives base/head independently, and uses task-owned build/profile directories.
Risk manifests must reproduce original Git bytes and match raw LLVM digests.
The same analyzer executable and tools measure both snapshots.

Candidate schema 1 records commit/base/run identity, per-stage status and wall
time, exact commands/exits, environment/tool metadata and artifact SHA-256s.
`verify` rejects mixed identities, missing stages, unsuccessful stages and changed
artifacts. A killed collector leaves an incomplete stage, which cannot verify.
The workflow uploads available raw logs, source archives, LLVM JSON/LCOV/Cobertura,
manifests, matrix runs and summaries with `always()`. Disposable build trees and
expanded snapshots are excluded from upload. Runner loss may prevent upload;
it can never convert a non-success job into aggregate success.

### Exceptions and acceptance

An exception must retain the issue, owner, approver, reason, expiry and
compensating controls defined by ADR-0025/GH-92. It documents a failure; it does
not waive thresholds or change CI status. Missing fields or expired exceptions
remain invalid. Selected hotspots cannot use an exception in place of evidence.

Every generated record is a **candidate**. CI has no accept flag or baseline
write authority. A repository owner must separately review the source/series
identity, raw artifacts, debt, measurement limitations and performance comparison
before accepting a candidate in an explicit baseline-review PR. Merging this
integration does not accept a baseline. Different metric series cannot be
numerically compared as a regression or silently replace historical results.

### Overhead and rollback

Stage/command wall times quantify collection cost on the actual runner. Local
reproduction of CI commands is identified as local; it is not a hosted runner
timing claim. Performance uses the existing uninstrumented fixture, five samples
and its unchanged 15% regression policy. Closure-instrumented builds must not be
used for product performance claims. See the [validation record](../quality/gh-96-validation.md).

If collection is broken, retain raw failed evidence and block acceptance while
repairing the tool. A reviewed revert may restore the previous workflow and
leave the extension explicitly unaccepted, while preserving the original
six-module gate and mandatory-path evidence. Do not lower thresholds, delete
mandatory rows, auto-accept a new series or change branch protection as rollback.

## Consequences

GH-115 implements generic baseline, debt and exception review in the standalone
[shadow policy engine](../quality/policy-engine.md), with
[local validation evidence](../quality/gh-115-validation.md). Legacy debt remains
explicit and exception metadata does not waive quality failures. This does not
accept a production baseline or replace the required gates decided here; Rust
adapter equivalence and migration remain separate OpenSpec tasks.

PR collection costs more than the previous fast PR workflow. Independent stages
and retained evidence make failures reviewable. The supported risk boundary is
deliberately restrictive until additional production sources have validated
measurement support. Linux CI evidence does not prove cross-platform execution
or real Docker/Podman behavior.
