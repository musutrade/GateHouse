# GH-291 hosted observations and evidence boundaries

The fixed main-push acceptance comparison retains the repository’s original three successful before runs—36023836720, 35716265337 and 37250306480—from `docs/quality/ci-topology/issue291-hosted-before.json`. They are compared with the three successful natural main pushes 37429831983, 37722931483 and 37725407263. The two failed runs 37411168218 and 37421759343, plus successful 37426739465, remain a separate recent diagnostic cohort; they do not replace or augment the fixed successful baseline.

| Measure | Original successful push-before (n=3) | Current successful push-after (n=3) | Reading |
|---|---:|---:|---|
| Time to Required Quality Aggregate | 1533s median (1533, 1416, 1785) | 1529s median (1583, 1529, 1484) | Same API endpoint in both cohorts: workflow `created_at` → Required Quality Aggregate job `completed_at`. Median difference after−before is −4s (−0.26%); descriptive only, not causal. |
| Non-skipped runner work (overlapping jobs summed) | 7586s median (7586, 7551, 8157) | 7636s median (7730, 7291, 7636) | Runner work, not wall time. |
| First observed job create→runner start | 6s cohort median of per-run first-start values (6, 8, 2) | Not comparable | Historical record retains first observed delay, not all-job medians. |
| All-job create→runner start | Not retained as per-run medians in original record | 2s median of per-run medians (3, 2, 2) | API start-delay proxy; does not measure dependency waiting or the full queue. |
| QPB runner elapsed | 1424s median of per-run platform medians (1462, 1295, 1424) | 1384s median of per-run platform medians (1482, 1384, 1289) | Includes setup, tool install, capture and upload. |
| QPB create→runner start | Not retained comparably in original record | 2s median of per-run medians (2, 2, 2) | Current API proxy only. |

The API lifecycle proxies `run_started_at→updated_at` are 1584s, 1530s and 1484s (median 1530s), kept separate from the exact Required Quality Aggregate comparison above. They are not used as `completed_at` or as a substitute for aggregate completion. The 4-second median difference is not evidence of causal speedup because source, tool/cache and runner inputs differ.

### Successful PR-after samples

The three valid successful PR-after runs are #298 `37721721679` (`fix/issue-287-text-redaction`) at `a5db1f099f8d982adaa9be5e5781f1d2b6e853f3`, #299 `37724107517` (`fix/issue-284-282-290-integrity`) at `3306b2aba5da9cbf4ff3e7c4a4a4bd9c2a4d9f2f`, and #300 `37726571334` (`fix/issue-289-baseline-delivery`) at `bb813b8b82e8b5636f82cde941354f9c8eea9948`. Branch names are the Actions API `head_branch` values, not the PR base branches. GitHub compare snapshots show each head contains GH-291 implementation commit `62c3d30ae3bb9d9afee312891c7a2ecf20b6af28`. The prior candidate IDs `34309500125`, `34311132169`, and `34312685548` are GH-165 runs from September 9, before GH-291, and are excluded from this cohort.

For PR #298–300, create→Required Quality Aggregate completion was 809s, 807s, and 879s (median 809s); summed non-skipped runner work was 2194s, 2161s, and 2347s (median 2194s). Per-run medians of all non-skipped job create→start delays were 1s, 2s, and 2s (cohort median 2s). Workflow create→first non-skipped real runner was 4s, 2s, and 117s (median 4s), a separate measure. Skipped virtual jobs are excluded. The PR-event Code Coverage and Quality Performance Baseline jobs were skipped under their push-only conditions; these runs are not used as tarpaulin parity evidence. Run #301 `37728180823` is supplementary only; a read-only GitHub PR API GET verified `base.ref=main`, `head.ref=fix/issue-288-toolchain-compatibility`, and head SHA `3d887a460bb72064b29a6f9ea71293e2949a16d7` (state closed at observation time). Its first real runner delay was 3s. The structured record stores `base_branch=main`; the run’s `head_branch` remains the API head branch.

### Full-repository coverage and tool setup

The quality-coverage-* artifacts retained elsewhere belong to the separate LLVM source-risk job, not tarpaulin; they are not associated with the Code Coverage job or treated as raw coverage output. The pre-change successful Code Coverage job was main run `37411168218`, job `112099699928`; three successful post-change natural-main Code Coverage jobs were run `37429831983`/job `112157846299`, `37722931483`/job `113134516115`, and `37725407263`/job `113142324870`. All four raw logs show cargo-tarpaulin 0.37.5 and the same locked LLVM/Cobertura command. Coverage was 10248/11647 (87.99%) before; the post-change runs report 10248/11647 (87.99%), 10249/11648 (87.99%), and 10249/11650 (87.97%). The first log shows source compilation/install in 117.790059s; the three after logs show checksum-verified prebuilt installation in 0.775141s, 0.443176s, and 0.412105s.

Each raw log shows Cobertura XML generation and an upload attempt. The non-blocking Codecov action step concluded success, but the server rejected tokenless upload (`Token required - not valid tokenless upload`). Thus these logs establish effective tarpaulin version, invocation/output, coverage figures, and observed installation interval; they do not establish Codecov ingestion or byte-identical XML. Independent parsing found all 92 per-file covered/total rows equal in the first before/after observation. This is same-version observed counter parity, without same-SHA controlled execution or raw XML byte parity. Later source changes have different denominators. These observations do not establish a causal total-CI speedup. PR-event coverage jobs were skipped and are not substituted for the main-push evidence.

### Recent natural-main diagnostics and platform observations

The existing API snapshots, artifacts, cache manifests and QPB logs are indexed in [the structured record](https://github.com/musutrade/Harness-Gate/blob/d9a932b23dcfea709175285b594ca200450a9971/docs/quality/ci-topology/issue291-natural-main-20261008.json) and retained under `/mnt/dev-ssd/dev-tmp/gh291-3before-3after/`; the four Code Coverage raw logs remain under `/mnt/dev-ssd/dev-tmp/gh291-natural-main-analysis-20261008/push-coverage-logs/`, with exact paths and hashes in the structured record. All 18 QPB image versions were parsed from the exact `##[group]Runner Image` through `##[endgroup]` block, never `Runner Image Provisioner`; original log SHA values remain unchanged.

| Platform | Diagnostic recent before: cold / five-warm / release-small build | Current successful after: cold / five-warm / release-small build | Records |
|---|---:|---:|---|
| Linux | 187.7s / 138.7s / 113s | 190.9s / 139.7s / 117s | Before 3 benchmark + 3 build; after 3 + 3 |
| macOS | 281.4s / 159.8s / 198s | 255.2s / 182.4s / 184s | Before 3 + 3; after 3 + 3 |
| Windows | 286.2s / 189.1s / 218s | 264.2s / 180.3s / 222s | Before 1 benchmark + 3 build; after 3 + 3 |

For each five-warm statistic, the five samples are first reduced to a per-run median; the cohort value is then the median of those run medians. Windows diagnostic-before has only one valid cold/five-warm benchmark record because two QPB jobs failed, while all three raw logs contain a release-small build record. The failures are retained as diagnostics, not successful baseline observations.

All six recent main-push QPB runs record Rust 1.99.0/Cargo 1.99.0 and Cargo.lock SHA `868ccabbd180e018f10128b31248c913487d40ef87fc7293d8fa485f1662e959`; fixture v1, harness v1, 601 scope paths × 100 matcher iterations and five warm samples are consistent. Source SHA, binary/configuration hashes and per-platform cache identities differ. Source-cache restoration is not a warm target build. Linux Python is 3.14.8 on 37722931483 versus 3.14.7 on the other recent samples.

Exact `Runner Image` versions: Windows `20260925.250.1`; macOS `20260907.0351.1`; Ubuntu `20260927.320.1`, except run 37722931483 at `20261004.327.1`. Main run 37729856085 is outside the fixed cohort and remains supplementary.

### Installer boundary supplement

The earlier documented `target/quality/issue291-tool-setup/` raw directory
could not be located. A new isolated run of pinned upstream installer
183e4297cca2404691e9380e1307288dced5c82a passed all five expected boundary
scenarios: unsupported-platform locked source fallback, fallback exit 42,
wrong effective version, corrupt checksum and failed download. Original
commands/status/stdout/stderr and download/Cargo traces are retained under
`/mnt/dev-ssd/dev-tmp/gh291-push-20261008/a5-installer-20261008T074405950771Z/`.
The independent audit checked all 311 files and 54 assertions at
`/mnt/dev-ssd/dev-tmp/gh291-audit-20261008/a5-installer-consumption/audit.json`.
Cargo/download stubs exercise installer control flow; this is not a second
actual source compilation or consumption of the missing historical logs.

These are bounded observations and new installer-boundary evidence. Final
submission still requires CI bound to its actual submitted SHA; historical
green runs are evidence inputs, not that new commit's status.
