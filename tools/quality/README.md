# Quality evidence and required CI gates

This directory contains Python CI tooling, ecosystem adapters, generic shadow
semantics and migration references. The [frozen product-boundary inventory](../../docs/quality/gh-146/python-boundary.md)
classifies every production module and records its callers, authority and final
disposition. These modules are not linked into, packaged with, or executed by
the `harness-gate` release binary. After GH-151 hosted acceptance, `harness-gate quality evaluate` owns authoritative
generic decisions and reporting in Rust. Python remains collection, CI and reference
tooling; final C-class module dispositions belong to task 7.
The [shared compatibility corpus](fixtures/generic-core/README.md) retains exact
inputs, full expected outputs and native/source bytes for that migration.

## Schema and fixture ownership

Each schema role has exactly one editable source: host contracts in
[`schema/`](../../schema/README.md), generic evidence and policy in
`tools/harness-gate/quality-core/schema/`, and plugin raw output in
[`tools/quality/schema/`](schema). The generic schemas here are byte-identical
generated copies, not a merge by filename; see
[schema ownership](../../docs/quality/schema-ownership.md).

Every fixture has one owner. Generic decision, evidence, policy and host-protocol
fixtures stay with core; only language/tool-specific collection fixtures belong
to GateProbes. See
[fixture ownership](../../docs/quality/fixture-ownership.md).

[GH-151](../../docs/quality/gh-151/README.md) records the exact hosted candidate
acceptance, retained comparisons, separate authority switch and rollback. The
opt-in differential workflow retains both engines. Existing required CI names,
dependencies and release authority remain unchanged.

Run from the repository root with Python >=3.12, stable Rust plus
`llvm-tools-preview`, cargo-nextest and cargo-llvm-cov **0.9.0** installed.
The collector uses a fresh workspace-local target; standalone commands should
also override an ambient shared/read-only Cargo target:

```bash
export CARGO_TARGET_DIR="$PWD/target/build"
python3 -m unittest discover -s tools/quality/tests -v
python3 -m py_compile tools/quality/*.py tools/quality/tests/*.py
python3 tools/quality/docs_consistency.py --output target/quality/docs-consistency.json

# Commit product source changes first; provide a full base SHA already in Git.
# CI supplies the PR base (or push before), the tested SHA, and run ID/attempt.
BASE_SHA=$(git rev-parse origin/main)
HEAD_SHA=$(git rev-parse HEAD)
RUN_ID=local-$(date +%s)
python3 tools/quality/ci_quality.py collect \
  --base-sha "$BASE_SHA" --head-sha "$HEAD_SHA" --run-id "$RUN_ID" \
  --output "target/quality/$RUN_ID"
python3 tools/quality/ci_quality.py verify \
  --base-sha "$BASE_SHA" --head-sha "$HEAD_SHA" --run-id "$RUN_ID" \
  --output "target/quality/$RUN_ID"
```

This is exactly CI's collection entry point and candidate schema 1. It retains
base/head/run identity, commands/exits/timings, four required stage results and
SHA-256 references to raw evidence. Existing output directories, stale artifacts,
missing base objects and incomplete collections fail. A candidate is never
accepted automatically. Full raw artifacts are uploaded with `always()`.

The stages preserve the six-module 80% gate, evaluate eleven production
boundaries and aggregate at 80%, run the versioned function-risk ratchet,
and collect isolated matrix evidence (all mandatory paths and >=95% applicable
rows). Changed production Rust files outside the supported risk boundary fail
with a measurement-review diagnostic; these reports do not certify whole-project
CRAP. See [ADR-0039](../../docs/adr/0039-required-risk-and-traceability-gates.md).

The required risk stage consumes the analyzer as a pinned release artifact
instead of compiling `tools/quality/rust-measure`. `rust-measure-release.json`
pins the `v0.4.8` asset, its SHA-256 and the exact
`harness-gate-rust-measure/0.3.1` / `mccabe-rust-3` identity; the download is
recorded in the candidate log and fails closed on a digest or archive mismatch.
The supplemental `rust-measure-native` workflow still builds the analyzer on
Windows and macOS for native validation until it moves to GateProbes.

GH-151 extends the inventory to `production-source-2`: `src/` and the linked
`quality-core/` are production, including the public replay module. The core is
an eleventh blocking boundary. Test-only modules retain explicit cfg(test)
exclusions. The unpublished `quality-replay/` comparator is migration tooling.
Declaration-only files without LLVM records remain production with pinned hashes;
no executable counters are synthesized. The `gh179-quality-configuration/1` risk
selection adds `config/mod.rs` and the four production `config/quality/` sources
to the previous `gh151-generic-production/1` selection on both commits. It uses
the unchanged analyzer 0.3.0 / mccabe-rust-3,
including vec repetition expressions. Both workspace packages are covered on the
base. Newly added sources are explicitly absent from its manifest and verified
against Git. The original selected functions, 80% line/region and CRAP <=30
thresholds, exact arithmetic, lineage and historical-debt rules are unchanged.
The quality model's declaration-only mapping is hash-pinned and AST-tested;
`config/quality/tests.rs` is an explicit test-only module, not production.
See [ADR-0041](../../docs/adr/0041-quality-configuration-v1.md) for the bounded
source-selection delta and its certification evidence requirements.

Individual diagnostic commands use the same report formats:

```bash
python3 tools/quality/coverage.py --output target/quality/coverage.json
python3 tools/quality/coverage.py --production \
  --raw target/quality/coverage.raw.json --lcov target/quality/coverage.lcov \
  --output target/quality/production.json
python3 tools/quality/critical_paths.py --collect \
  --evidence target/quality/critical-path-runs/bundle.json \
  --output target/quality/critical-paths.json
python3 tools/quality/measurement_contract.py
python3 tools/quality/contracts.py
python3 tools/quality/benchmarks.py --output target/quality/benchmarks.json --samples 5
```

The matrix rejects aggregate nextest JSONL/module coverage as a replacement for
isolated evidence. The critical-path collector builds instrumented binaries once
per collection and runs up to two isolated tests concurrently by default; pass
`--jobs 1` for serial execution or `--jobs N` for 1 to 8 workers. Local timing
observations are in the
[collection benchmark](../../docs/benchmarks/critical-path-collection/README.md).
The collector continues other stages after an ordinary gate
failure, retains the failure and exits nonzero. Candidate timings describe this
collection environment; hosted CI overhead comes from the uploaded CI candidate.
This repository has no project-local `.harness-gate/flow.toml` declaring `ci`:
`harness-gate config check` and `harness-gate verify --profile ci --all` are not
applicable here.

The helper tests and bytecode compilation run in the `Quality Script Tests`
CI job and are included in `Required Quality Aggregate`. Quality scripts are
reviewed as production-like policy code: a failing or untestable script cannot
silently approve a release.

`measurement_contract.py` records the commit, target directory/triple, Cargo
profile, tool versions, exact nextest test-binary paths and digests, and the
`cargo-llvm-cov` instrumentation environment. Its child-process rule is
explicit: `LLVM_PROFILE_FILE` is inherited, but a child executable contributes
coverage only when that executable was built with `-C instrument-coverage`.
The command reports branch coverage as `unsupported` unless the baseline command
is explicitly changed to request branch instrumentation. The output is
candidate evidence, not an automatic baseline acceptance.

## Complexity analyzer and quality evidence

The locked development complexity analyzer
(`complexity_analyzer.py`, identity `harness-gate-complexity` 0.1.0, MIT)
is a stdlib-only, in-repository Python tool used for quality evidence only. It
is never linked into the release binary. Its frozen rule is `mccabe-rust-1`
version 1, and its supported Rust fixture subset, raw-count contract, series
identity, and source-symbol identity are documented in
[`docs/quality/complexity-analyzer.md`](../../docs/quality/complexity-analyzer.md).
The versioned machine schema is
[`schema/quality-evidence.schema.json`](schema/quality-evidence.schema.json).

Regenerate the candidate evidence and validate it:

```bash
python3 tools/quality/complexity_analyzer.py \
  --source tools/quality/fixtures/complexity/controls.rs \
  --source-root tools/quality/fixtures/complexity \
  --output target/quality/complexity/controls.json
python3 tools/quality/quality_evidence.py validate \
  --record target/quality/complexity/controls.json
python3 tools/quality/complexity_analyzer.py \
  --source tools/quality/fixtures/complexity/controls.rs \
  --source-root tools/quality/fixtures/complexity \
  --output target/quality/complexity/controls-again.json
python3 tools/quality/quality_evidence.py compare-series \
  --record target/quality/complexity/controls.json \
  --record target/quality/complexity/controls-again.json
```

`compare-series` rejects records whose canonical series key differs, so a
generated record can only be compared with the committed expected fixture when
their toolchain fields match; the unit tests normalize target/Python and drop
`commit` to compare fixture structure reproducibly. Unsupported Rust syntax
raises `ComplexitySyntaxError` and the analyzer exits non-zero, so the frozen
subset fails closed instead of silently widening.

The benchmark runner executes the checked-in no-network fixture in both serial
and opt-in parallel modes. Its JSON keeps per-mode raw samples, the configured
concurrency limit, fixture-observed peak concurrency, report-derived step
timings, scheduler overhead, and the shareable-service startup/reuse counts.
A median comparison is under
`verification.serial`, `verification.parallel`, and `verification.comparison`.

The timed `cargo nextest` samples exclude the `benchmark_fixture_test`
integration probe (recorded as `tests_seconds.excluded_binaries`). That probe
launches the product's own parallel benchmark, so running it inside the timed
sample would nest the benchmark under the same constrained runner and make the
sample flaky instead of measuring the suite. The probe stays required in the
`Test` and `Quality Script Tests` jobs. A failed cold/warm sample retains its
full stdout and stderr under `target/quality/nextest-samples/`, and the failure
message keeps both the output head (the probe diagnostic) and tail.

`post_remediation_benchmarks.py` is a local evidence generator for the R-16
wait/backoff and scheduler scenarios and the R-17 validation-allocation
change. It alternates samples between a pre-change binary (for example the
v0.3.5 or pre-hardening commit) and the post-change v0.3.6 binary, then writes
per-sample JSON and reviewable Markdown under `docs/benchmarks`. It is a
manual review tool; the periodic quality-baseline workflow intentionally does
not run it.

`contracts.py --accept` writes the Linux textual golden snapshot and is a
reviewed local operation; CI never passes that flag. `contracts.py --structured`
is used for macOS and Windows to assert exit status, error code, reports, and
the no-ANSI policy without accepting platform-specific text.

The scheduled `Refresh Quality Baseline` workflow creates a pull request for a
new candidate baseline rather than rewriting a canonical result on its own.
Review its JSON, Markdown, and uploaded raw reports together before merging.

### PR delivery and an existing candidate retry (GH-289)

The selected and user-approved delivery mode is **PR**, including scheduled
captures. Repository Actions PR creation was separately authorized, but its
setting update was rejected with enterprise HTTP 409. This code changes no
repository/organization setting and substitutes no token. An enterprise
administrator must resolve that restriction before actual Actions PR delivery
can be certified. Simulation tests do not establish that the restriction is gone.
Artifact-only delivery is not selected; the workflow never silently changes to
that mode. A diagnostic upload remains a candidate record, not baseline adoption.

Before installing Rust or collecting samples, `baseline_delivery.py precheck`
reads the Actions workflow permission setting. Explicit `false` returns `denied`
and exits nonzero; `true` reports `policy-enabled`, which does not guarantee a
later PR POST will succeed. Unreadable, malformed, forbidden, or timed-out reads
report `unknown`, never precheck PASS, and retain PR intent. The workflow's
explicit job permissions include `actions: read`, alongside the original capture
contents/PR writes; retry needs only contents read and PR write. The setting GET
may require administration read unavailable to `GITHUB_TOKEN`; unknown is an
expected possibility, not a reason to add an administrator token. See GitHub's
[permission API](https://docs.github.com/en/rest/actions/permissions#get-default-workflow-permissions-for-a-repository)
and [Actions settings](https://docs.github.com/en/repositories/managing-your-repositorys-settings-and-features/enabling-features-for-your-repository/managing-github-actions-settings-for-your-repository).

Capture still runs the original `benchmarks.py --samples 5`, uploads
`target/quality` with `always()`, and uses `create-pull-request@v7` to commit/push
the candidate. The raw upload retains its warning when failure diagnostics do
not exist; a separate blocking step requires successful capture/upload and a
real artifact ID and SHA-256 output before any PR action. Missing artifacts
cannot become successful delivery. An always-running read-only summary reports
the original run/attempt, measured SHA, existing candidate SHA/parent, artifact
ID/digest, and exact dispatch inputs. Missing identities are explicit blockers
and summary exits nonzero. It never reconstructs a missing candidate branch.
New capture artifact names include the attempt. Original raw files and failed
capture diagnostics are retained without requiring a new delivery manifest.

After the permission problem is resolved, use the original summary's values:

```bash
gh workflow run quality-baseline-refresh.yml \
  -f operation=delivery-only \
  -f source_run_id=ORIGINAL_RUN_ID \
  -f source_run_attempt=ORIGINAL_ATTEMPT \
  -f candidate_sha=EXACT_EXISTING_CANDIDATE_SHA \
  -f artifact_id=ORIGINAL_RAW_ARTIFACT_ID \
  -f artifact_digest=sha256:ORIGINAL_ARCHIVE_SHA256
```

Delivery-only checks out the trusted default-branch helper, then reads API data
and candidate blobs. It does not execute candidate code, install measurement
tools, recollect metrics, create commits, rebase, force-push, or accept a baseline.
The original run must be a same-repository default-branch scheduled/manual
baseline workflow. Its exact attempt jobs must show successful capture/upload.
The fixed `automation/quality-baseline-RUN_ID` branch must equal the supplied SHA,
have the measured SHA as its sole parent, and change exactly `current.json` and
`current.md`. `current.json.commit` must match that original measured SHA. The
original tool versions, target, series, samples, and comparison bytes remain
unchanged. Concurrent delivery attempts are serialized by original run ID, and
the remote branch is checked immediately before and after PR delivery. A changed
branch or a failed post-check is a delivery failure, even if a PR was created.

Artifact verification binds API run/repository/HEAD identity, archive digest,
attempt-specific job steps, and upload timestamps. Legacy names without an
attempt are accepted only when the upload window uniquely identifies one of at
most 20 attempts. Ambiguous/unavailable attempts, expired/missing artifacts,
unverifiable source or changed bytes block delivery. API pagination is limited
to ten 100-entry pages; ZIPs are limited to 32 MiB compressed, 128 MiB expanded,
and 4096 entries before reading entries. No archive is extracted. Absolute,
parent, backslash, ambiguous/duplicate paths, file/directory collisions, links,
special files, and encrypted entries are rejected.

The helper verifies the existing five warm sample values, five serial reports,
five parallel reports, five scope records, report/summary correspondence,
retained log references, concurrency records, and command results when present.
It records hashes for original raw bytes and both candidate files without
recomputing metric formulas. Historical artifacts have no delivery manifest;
some have no verification `command-result.json` records. Those absences are
explicitly reported, not retroactively filled. This format does not retain warm
command evidence, so delivery does not certify those commands' execution. Missing
required existing reports/logs/references still block delivery.

Capture and retry PR bodies carry one `baseline-delivery-v1` JSON comment with
repository, source run/attempt, measured SHA, artifact ID, and archive digest.
Retry can create a PR or reuse one only after exact field/type/unique-comment
matching, plus head/base/repository/SHA checks. A missing or conflicting existing
PR association blocks reuse; the helper does not edit it or infer provenance
from a substring. Historical candidates without a PR can receive a new correctly
associated PR after verification. Delivery JSON and step summaries retain the
original identities and `baseline_accepted: false`; retry diagnostics upload
even on failure. Human review of candidate changes/raw evidence and all required
gates remain necessary before adoption.

Run the bounded simulation fixtures with retained evidence in a new directory:

```bash
BASELINE_DELIVERY_TEST_EVIDENCE=/path/to/new/evidence \
  python3 -B -m unittest discover -s tools/quality/tests \
  -p test_baseline_delivery.py -v
```

Fixtures create actual two-commit Git candidates and ZIP bytes, then simulate
GitHub reads and PR POST responses. They keep each invocation's parameters,
status, output, and API calls, including failed delivery followed by same-SHA
retry. They verify rejection of permission, identity, attempt, artifact, sample,
raw-reference, ZIP, conflicting-PR, and branch-movement failures. They establish
no actual hosted PR permission. Engineering Policy measurement formulas,
thresholds, five-sample capture, series identity, required gates, baseline/ratchet,
and review/adoption authority are unchanged.

## Baseline Exceptions

An exception never converts a failed quality result into a pass. It is a
temporary review record that must include:

- a tracked issue or pull request;
- one accountable owner;
- the measured failure and a concrete rationale;
- an expiry date; and
- approval from a repository code owner.

The exception is recorded in the pull request description and linked from the
evidence summary. The threshold, raw result, and aggregate check remain
unchanged. At expiry the owner either lands a fix, submits a reviewed baseline
change, or closes the exception; expired exceptions cannot be carried forward
silently.

The first accepted baseline is recorded in
`docs/benchmarks/phase-1/README.md`. New candidates are accepted only after
the `Required Quality Aggregate` check and the corresponding raw artifacts
have been reviewed.

The CI workflow keeps coverage, contract, benchmark, and documentation jobs
independent for diagnostics, then runs `quality-required` with `always()`. That
aggregate job fails closed when any dependency fails, is cancelled, or is
skipped, and is the single check to select in repository branch protection.

## Standalone generic project model

The GH-111 [project model](../../docs/quality/project-model.md) and
[migration compatibility inventory](../../docs/quality/migration-compatibility.md)
implement only OpenSpec tasks 0.3 and 1.1–1.4. Synthetic fixtures need no language
toolchains. The current Rust required gates remain authoritative; this model does
not evaluate policy or collect generic evidence.

## Shadow project contracts and reporting

[`project_report.py`](project_report.py) checks an opt-in
`.harness-gate/project.json` and emits project/component/gate reports from retained
normalized evidence. [Configuration, contract metrics and CLI examples](../../docs/quality/project-reporting.md)
include a single Rust topology and a synthetic Angular + Rust + Python + Java
project whose green local gates are blocked by a breaking API contract. These
fixtures do not certify additional adapters or replace required Rust CI.

## Equivalent main push checks

The [CI inheritance amendment](../../docs/engineering-policy.md#equivalent-main-push-check-inheritance)
permits three checks to inherit verified PR success on an equivalent main merge:
Linux tests, native macOS/Windows tests and quality coverage. `ci_reuse.py` verifies
the original run and a small PR identity receipt; it does not rewrite or republish
quality evidence. Classify logs report the original run/attempt and reason for
reuse or full execution. Missing or mismatched proof runs all checks normally.
PR checks always retain their normal routing. Producer changes, including first
deployment, cannot inherit results. The aggregate accepts only these explicitly
authorized skips; all failures and missing dependencies still block.
