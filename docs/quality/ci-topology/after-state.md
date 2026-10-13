# GH-169: Hosted after-state acceptance and rollback

This record implements only `optimize-ci-execution-topology` tasks 6.1–6.4,
after GH-168. The retained optimization demonstrates meaningful reduction in
avoidable tool-install work with assurance parity. It does **not** establish a
causal improvement in PR latency or total runner cost. Overall proposal closure
and the submitted commit's Required Quality Aggregate remain pending; task 7
is not checked off by this record.

## Hosted cohort and reproducibility

The [GH-164 baseline](baseline.md) has three successful PR runs, all attempt 1:
34303413318, 34301967575 and 34291282719. The primary after cohort comprises both
successful PR runs after GH-167's last performance edit:

| Hosted PR run | PR | Required Quality Aggregate | PR head | Tested merge commit |
| --- | --- | --- | --- | --- |
| [34312685548](https://github.com/musutrade/Harness-Gate/actions/runs/34312685548) | #175 / GH-168 | success, 7 s | 3028c40e6a8d21b3634c8cc4c87dcabbf4dd21ab | 388a69b8b30e9032dfecc6f90d56e9e86e7374d5 |
| [34311132169](https://github.com/musutrade/Harness-Gate/actions/runs/34311132169) | #174 / GH-167 | success, 7 s | 8594a9ae441fbb3b09b33f6c9808d80d3792f766 | 2ae900cf615ae5c35f02bd64250151dcb6dac9d2 |

Both use the same executable workflow. GH-168 adds two aggregate comments and
hardens malformed input handling, with no change to successful execution.
Their workflow blobs differ and are retained explicitly, not treated as the same
source identity. PR head, tested merge HEAD_SHA, collection BASE_SHA, RUN_ID,
workflow blob at both heads, run/attempt, job/step timestamps, labels, conclusions
and source URLs are in [hosted-after-input.json](https://github.com/musutrade/Harness-Gate/blob/d9a932b23dcfea709175285b594ca200450a9971/docs/quality/ci-topology/hosted-after-input.json).
[Log excerpts and full-log SHA-256s](https://github.com/musutrade/Harness-Gate/blob/d9a932b23dcfea709175285b594ca200450a9971/docs/quality/ci-topology/hosted-after-log-audit.json) retain effective
tool versions, collection identities, cache decisions, nested action timings,
artifact digests and compressed upload sizes. Capture used GitHub's run,
attempt-jobs, contents and job-logs APIs on 2026-09-09; no runs were dispatched.

The GH-165 and GH-166 successful runs 34307555998 and 34309500125 are retained in
[stage input](https://github.com/musutrade/Harness-Gate/blob/d9a932b23dcfea709175285b594ca200450a9971/docs/quality/ci-topology/hosted-stage-input.json) and [stage normalization](https://github.com/musutrade/Harness-Gate/blob/d9a932b23dcfea709175285b594ca200450a9971/docs/quality/ci-topology/hosted-stages.json)
for rollback diagnosis only. Their 737/666-second critical paths are excluded
from the primary after cohort; selecting them would overstate the latency gain.
Two latest runs are a small sample with changing commits, hosted contention and
uncontrolled toolchain/cache state, not matched experimental repetitions.

Reproduce the committed records from repository root:

```bash
python3 tools/quality/ci_timing.py --input docs/quality/ci-topology/hosted-after-input.json --output docs/quality/ci-topology/hosted-after.json
python3 tools/quality/ci_timing.py --input docs/quality/ci-topology/hosted-stage-input.json --output docs/quality/ci-topology/hosted-stages.json
python3 tools/quality/ci_comparison.py --output docs/quality/ci-topology/hosted-comparison.json
```

Normalization remains GH-164 v1: PR workflow run creation to aggregate completion, active
job walls from start/end timestamps, and summed runner wall minutes without
billing multipliers or queue time. Only new step-name classification was added:
`Configure Cargo state` is setup; sealing and verifying artifacts are artifact
work. Post actions remain cleanup. The old baseline reproduces exactly, including
its input hash. Nested cache and upload action times below already lie inside
setup/cleanup; adding them to job totals would double count.

## Comparison

All figures below come from [the generated comparison](https://github.com/musutrade/Harness-Gate/blob/d9a932b23dcfea709175285b594ca200450a9971/docs/quality/ci-topology/hosted-comparison.json).
Seconds summed across concurrent steps are work, not elapsed PR latency.
| Measure | Before median (range) | After median (range) |
| --- | --- | --- |
| PR workflow run creation → aggregate completion, seconds | 903.0 (846.0–1141.0) | 878.5 (875.0–882.0) |
| Total runner wall minutes | 40.7 (37.5–43.2) | 39.3 (38.3–40.3) |
| setup seconds | 195.0 (183.0–197.0) | 172.5 (170.0–175.0) |
| tool install seconds | 423.0 (392.0–453.0) | 12.0 (12.0–12.0) |
| quality collection seconds | 600.0 (560.0–821.0) | 836.5 (828.0–845.0) |
| execution seconds | 1071.0 (997.0–1175.0) | 1243.0 (1189.0–1297.0) |
| artifact seconds | 10.0 (8.0–11.0) | 10.5 (10.0–11.0) |
| cleanup seconds | 11.0 (9.0–67.0) | 49.0 (47.0–51.0) |
| Linux runner minutes | 28.6 (28.2–33.3) | 26.9 (26.2–27.5) |
| macOS runner minutes | 4.3 (3.2–4.9) | 5.1 (4.8–5.3) |
| Windows runner minutes | 5.7 (5.7–7.5) | 7.4 (7.3–7.5) |

| Job wall seconds | Before median (range) | After median (range) |
| --- | --- | --- |
| Build | 153.0 (140.0–154.0) | 157.5 (149.0–166.0) |
| Clippy | 47.0 (42.0–48.0) | 46.5 (39.0–54.0) |
| Documentation Consistency | 55.0 (52.0–59.0) | 73.0 (65.0–81.0) |
| Format | 14.0 (13.0–15.0) | 15.5 (15.0–16.0) |
| Generic Quality Shadow | 22.0 (22.0–27.0) | 22.0 (21.0–23.0) |
| Quality CLI Contracts | 59.0 (56.0–62.0) | 66.0 (64.0–68.0) |
| Quality Coverage and Critical Paths | 858.0 (796.0–1127.0) | 863.0 (856.0–870.0) |
| Quality Script Tests | 108.0 (99.0–120.0) | 112.5 (101.0–124.0) |
| Release Governance Contracts | 7.0 (7.0–8.0) | 9.0 (8.0–10.0) |
| Required Quality Aggregate | 7.0 (6.0–9.0) | 7.0 (7.0–7.0) |
| Security Audit | 193.0 (191.0–200.0) | 16.5 (11.0–22.0) |
| Test | 222.0 (185.0–226.0) | 224.0 (187.0–261.0) |
| Test (macos-latest) | 256.0 (195.0–296.0) | 305.5 (291.0–320.0) |
| Test (windows-latest) | 342.0 (339.0–450.0) | 442.0 (436.0–448.0) |

Tool installation drops by 411 median summed seconds, from 423 to 12. The audit
job falls from 193 to 16.5 seconds. Baseline workflows forced three PR source
installs: audit, quality nextest and quality llvm-cov. Both after logs show the
pinned prebuilt versions (nextest 0.9.143, llvm-cov 0.9.0, audit 0.22.2), removing
those source builds in these samples. Native test nextest was already prebuilt;
no additional compilation saving is credited to it. Locked source fallback
remains possible, so 12 seconds is an observation, not an installation SLA.

Quality collection remains the last required child in every primary sample.
Its collection step increased from 600 to 836.5 median seconds while the whole
quality job remained approximately flat: saved installation time was absorbed by
longer collection. The critical-path ranges overlap, as do total runner minutes;
macOS and Windows work increased. No guaranteed percentage, causal PR speedup,
or billing saving follows from these measurements.

Compilation-capable product jobs remain separate: Linux/native test, release
Build, Clippy, dev contracts, docs, script fixtures and instrumented quality
collection. Zero target-cache hits means no observed product compilation was
eliminated by the compiled-cache experiment. Base/head and isolated critical-path
builds remain distinct measurement identities; LLVM exports and downstream
projection reuse retained counters without recollection. No job merge, native
replacement or cross-profile binary reuse is credited as a saving.

Normalized artifact steps are essentially flat at 10 versus 10.5 median seconds.
New Cargo boundary uploads additionally occupy 7.750/9.000 summed nested seconds
in setup. All uploaded zips total 38,876,488/38,874,272 bytes in the two after runs;
there is no before-byte comparison retained, so no byte-saving claim is made.
The informational release binary upload now actually finds its resolved output;
that added payload is not authoritative quality/release evidence. Sealing,
independent digest transport and full inventory verification are retained despite
their overhead because they remove artifact identity ambiguity.

## Individual rollback decisions (task 6.4)

- **Retain pinned tool acquisition.** Hosted installation work falls materially,
  with unchanged tool contracts, checksum verification, explicit version checking,
  locked fallback and blocking failures. The reduction is independently visible
  in audit and quality setup; it does not depend on the reverted experiments.
- **Disable compiled target caching for every caller.** Each primary run has
  seven misses and zero hits, with 41.964/32.908 summed seconds spent restoring
  and saving those caches. GH-166's diagnostic sample also has seven misses and
  zero hits (33.998 seconds). Broad configuration identities change as CI tooling
  changes; weakening keys to force hits would introduce ambiguity. The composite
  now defaults to false, and a regression prohibits callers enabling it. Keep
  explicit target discovery, class/OS isolation and small identity records. Keep
  source-only caching: the latest sample has nine source-cache hits; these caches
  contain no tools, product binaries or authoritative measurements.
- **Revert docs build-once execution.** The docs operation takes 43 seconds in
  each primary sample, versus 42 median (34–44) in the baseline and 42 in GH-166.
  No useful hosted benefit is established for eliminating ten Cargo startups.
  Restore locked `cargo run` for all eleven operations. Keep failure injection
  for each preset, migration, missing secrets and schema drift, and preserve
  all report fields. Do not describe repeated Cargo startups as eleven complete
  recompilations: Cargo's freshness checks still reuse the within-job target.

These rollbacks are conservative decisions on measured experiments. The current
rollback commit has no hosted timing sample yet, and this report does not
subtract cache overhead from PR latency to invent a projected result. The
controller will run Required Quality Aggregate for the submitted SHA.

## Semantic parity review (task 6.3)

The [source comparison](https://github.com/musutrade/Harness-Gate/blob/d9a932b23dcfea709175285b594ca200450a9971/docs/quality/ci-topology/semantic-parity-source.json) binds the latest baseline
PR head and GH-168 PR head, lists changed files, retains reviewed boundary
patches and records identical blob IDs for policy, measurement, Rust product/
quality-core and schema sources. The comparison has 39 changed files and is
not truncated. This is source evidence combined with successful hosted outcomes
and local negative tests, not a claim that green CI alone proves all semantics.

| Boundary | Review and evidence |
| --- | --- |
| Required outcomes and platforms | Frozen `fixtures/ci-topology.json` and `test_ci_topology.py` cover full Linux/macOS/Windows nextest, schema sync, audit, fmt, Clippy, build, quality coverage/risk/critical paths, CLI contracts, docs, release governance and Python scripts. Event conditions, dependencies and required check names remain stable. Push retains native builds/contracts, coverage and performance; no push timing claim comes from PR-only samples. |
| Fail-closed aggregation | GH-164 corrected the pre-existing stale PUSH_ONLY classification of already-required native PR tests; this strengthens enforcement rather than preserving the old gap. GH-168 rejects malformed needs and children. Tests exercise every required child with missing, malformed, failed, cancelled or skipped results on both events, allowed PR push-only omissions, advisory shadow behavior, unsupported events and a frozen step allowlist. Aggregate only evaluates outcomes; no compilation, tests or collection are added. |
| CRAP, coverage and critical paths | Engineering Policy, source coverage/risk/complexity/critical-path collectors, thresholds, debt/ratchet rules, manifests and Rust quality decision code have identical blobs. CRAP <= 30, supported source boundaries and mandatory isolated critical-path rows are unchanged. `ci_quality.py` collection/verification code is unchanged; only aggregate classification/validation differs. |
| Measurement identities | Quality remains one producer per series; base/head source snapshots and isolated targets remain separate. Collection log BASE_SHA/HEAD_SHA/RUN_ID are retained independently of PR head. No baseline reset, coverage export rerun, instrumented-to-performance substitution or projection recollection is introduced. Warm/cold benchmark and release-small contracts remain push-only and unchanged. |
| Cache and artifact trust | Source cache contains downloads only. Compiled caches are now disabled. Contracts always rebuild with locked Cargo and discover the real target. Artifact manifest binds repository/workflow/producer, checkout, run/attempt, configuration, tools and file hashes. Consumer requires an independently transported manifest digest and exact inventory before projection. Negative tests reject missing output, wrong identity, altered/extra/missing files, symlinks and mutable builds. Integrity checks do not claim cryptographic producer authentication. |
| Authority and release | Existing Rust risk/generic/release decisions remain authoritative under ADR-0039/0040. Generic Shadow remains advisory and cannot recollect/reseal. Release workflow changes only pinned audit installation; lockfile scope and deny-warnings remain. The Linux Build upload is informational and creates no publishing authority. |
| Docs rollback | All four init/check preset pairs, v1 migration/check/secrets, exported schema bytes, machine/manifest/registry schemas, links, bilingual docs, policy anchors and bounded sandbox wording remain checked. Each CLI operation still fails closed. |

See [local validation](https://github.com/musutrade/Harness-Gate/blob/d9a932b23dcfea709175285b594ca200450a9971/docs/quality/ci-topology/after-validation.json) for exact results and environment
limitations. Governing records remain [Engineering Policy](../../engineering-policy.md),
[ADR-0039](../../adr/0039-required-risk-and-traceability-gates.md) and
[ADR-0040](../../adr/0040-language-agnostic-evidence-policy.md). No policy delta
or overall OpenSpec acceptance is declared by this issue.
