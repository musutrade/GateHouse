# GH-291: Profiled oracle comparison and pinned coverage setup

This is a bounded implementation milestone for [GH-291](https://github.com/musutrade/Harness-Gate/issues/291),
not issue closure or proof of a hosted latency reduction. The baseline is
`64c6c59c30bbb76884cd767b8ae0ea35ed91d747`. Engineering Policy semantics,
coverage/CRAP identities, requiredness, failure propagation and Rust authority
are unchanged. No policy delta is proposed.

## Profile first, optimize a demonstrated cost

The local [profile record](https://github.com/musutrade/Harness-Gate/blob/d9a932b23dcfea709175285b594ca200450a9971/docs/quality/ci-topology/issue291-profile.json) retains source hashes, interpreter
and platform, wall/process/child CPU times, startup probes, function call counts,
selected cumulative/self times, full-profile hashes, every fresh-process sample
and exact output-parity hashes. The opt-in driver retains complete cProfile data,
full function statistics, stdout/stderr and materialized inputs for each run:

```bash
python3 tools/quality/fixtures/generic-core/profile_oracles.py --samples 3 --output target/quality/oracle-profile
```

The output directory must be new. Each sample starts a new Python process and
materializes independent roots; it never consumes an earlier oracle result.
The driver reports failure with retained logs and a nonzero exit. Profiled runs
and ordinary fresh-process runs are separate. Child CPU is unavailable (`null`)
on platforms where `os.times()` does not supply it. Interpreter startup is a
separate empty-process probe and is not subtracted from measured workloads.

Primary measurements below use Linux tmpfs work roots and Python 3.12.14. They
are diagnostic observations on a shared host with uncontrolled OS page caches,
not hosted measurements. An exploratory after run on an overlay filesystem is
retained separately and excluded from comparisons. cProfile adds substantial
Python overhead: do not use its speedup ratio as a CI speedup prediction.

| Profiled operation | Before seconds | After seconds | Calls before / after |
| --- | ---: | ---: | ---: |
| Policy oracle total wall / process CPU | 66.195 / 63.834 | 42.557 / 42.051 | one fresh process each |
| Policy frozen-output comparison | 19.814 | 0.626 | 63 / 63 |
| Policy evaluation | 37.032 | 33.292 | 262 / 262 |
| Policy fixture copytree | 1.105 | 1.146 | 3814 / 3814, including recursion |
| Policy byte writes | 0.082 | 0.093 | 1799 / 1799 |
| Policy child creation | 0.003 | 0.003 | 6 / 6 |
| Retained replay total wall / process CPU | 49.516 / 49.488 | 29.783 / 29.773 | one fresh process each |
| Replay frozen-output comparison | 18.094 | 0.545 | 33 / 33 |
| Replay materialization | 0.253 | 0.260 | 48 / 48 |

Cumulative times overlap and must not be added. Semantic evaluation remains the
largest cost; changing frozen implementations or skipping it is not part of this
change. Copying, materialization and child startup are smaller on this host.
The startup probes were 0.018 seconds before and 0.014–0.015 seconds after.

Fresh-process, **uninstrumented** worker samples were policy 16.327 seconds and
replay 12.683 seconds before; after policy 15.088, 14.716, 14.776 seconds and replay
11.315, 10.801, 11.795 seconds. Three earlier direct-process baseline samples
(policy 24.837, 26.744, 16.305; replay 15.915, 14.823, 11.939 seconds) are retained
with their original method. Their variability prevents a causal whole-workload
percentage claim. Hosted before/after evidence remains required.

## Minimal change and semantic parity

`replay.same_json` uses compact, sorted UTF-8 JSON only for transient equality.
Pretty indentation prevents CPython's accelerated encoder from handling these
large trees. Removing that formatting work retains serialized JSON types,
finite-number requirements and UTF-8 validation. Ordinary Python equality is
not used because it conflates `True`, `1` and `1.0`. Both operands are still
serialized, including when they reference the same object, so invalid values
cannot bypass validation. No semantic result, input, schema or artifact is cached.

`canonical()` and every persisted byte format are unchanged. `oracle_matches`
keeps its precise missing-file wording exception; `authority.compare` retains
all differing leaves, JSON Pointer paths and diagnostic classifications.
All frozen C-class modules, corpus inputs, expectations, manifest, external
schemas and production Rust remain unchanged. Every original negative fixture,
mutation and independent per-case root remains executed.

- All 826 policy comparisons from 31 reference tests are byte-identical after
  replacing only each run's temporary work root with `$WORK`:
  `6d63f97d6db2dd694e65f4e202eaf01858c07a2795b44a3f60bd9a7116fd215a`
- All 33 frozen replay results are exactly byte-identical:
  `60c331f6570dc2b749a0161df380b3de00dfc37a2b76b971513bdd340b3ff437`
- Focused tests cover reordered objects, nested types, signed zero, Unicode,
  invalid numbers/objects/cycles/surrogates, unchanged canonical bytes, every
  preaccepted missing-file variant and rejected permission/path changes
- Driver tests cover complete/failing records and refusing an existing output

## Secondary runner work: coverage tool acquisition

The existing [coverage job](https://github.com/musutrade/Harness-Gate/actions/runs/37411168218/job/112099699928)
installed tarpaulin 0.37.5 from source in about 118 seconds and measured
10248/11647 lines (87.99%). Pinning **that same version** removes version drift.
The updated immutable installer supports its checksummed official prebuilt
binary, preserves locked source fallback for unsupported binaries and rejects
checksum/download/version failures. Effective version checking handles the
release's actual `cargo-tarpaulin-tarpaulin 0.37.5` output.

See [tool setup](tool-setup.md) for upstream source/checksum identity, unchanged
installer scripts and existing tool pins, real prebuilt-install smoke and
controlled fallback/failure tests. The exact `--engine llvm` coverage command,
XML output and upload remain unchanged. Installed-tool caching is not enabled.
This reduces installation work; coverage was not the measured critical child,
so no corresponding end-to-end saving is claimed.

## Hosted evidence and remaining acceptance

[Historical observations](https://github.com/musutrade/Harness-Gate/blob/d9a932b23dcfea709175285b594ca200450a9971/docs/quality/ci-topology/issue291-hosted-before.json) retain run/attempt, API head SHA, independently logged checkout SHA, PR quality
collection base/head (synthetic merge) SHAs, job/step timestamps, runner labels, critical-job tool/cache log excerpts and
full-log hashes for three successful PR and three successful push runs. They
are separate source/toolchain/cache cohorts, not matched repetitions:

| Event | Runs | Creation to aggregate seconds |
| --- | --- | --- |
| PR | 36022253329, 35715449453, 37408616833 | 756, 529, 760 |
| Push | 36023836720, 35716265337, 37250306480 | 1533, 1416, 1785 |

The critical child is quality collection for those PRs and Windows performance
baseline for those pushes. September uses Rust 1.98.1; the newer runs use 1.99.0.
The latest successful Windows push has a source-cache miss while the other two
restore it. These differences are recorded and cannot be attributed to this
patch. Summed runner work excludes skipped jobs and does not add overlapping
steps. First observed job-start delay is reported separately; it is not a full
measurement of dependency waits or scheduler queueing.

Main push 37411168218 failed the separate heartbeat test in Windows baseline.
It is retained as a **censored diagnostic**, with no successful critical-path
value. A shorter failed run must not count as a performance improvement.

The three successful after PR runs (37721721679, 37724107517,
37726571334) and three successful after push runs (37429831983,
37722931483, 37725407263) are now retained and independently reviewed.
PR creation-to-aggregate median was 756s before and 809s after; push median
was 1533s before and 1529s after. These mixed source/compiler/cache cohorts
do not establish a causal overall speedup. First real PR runner delays are
4/2/117s, median 4s; skipped virtual jobs are excluded.

Tarpaulin 0.37.5 installation was 117.790059s from source before and
0.775141/0.443176/0.412105s for verified prebuilt installation after.
The first before/after logs have identical covered/total counters for all
92 files. This is same-version observational parity, without same-SHA
controlled execution or raw XML byte parity. Later denominators differ.
Codecov rejected tokenless transport; nonblocking action success does not
prove ingestion. Separate source-risk artifacts are not tarpaulin output.

See [the hosted evidence appendix](issue291-natural-main-appendix.md) and
[structured run/identity record](https://github.com/musutrade/Harness-Gate/blob/d9a932b23dcfea709175285b594ca200450a9971/docs/quality/ci-topology/issue291-natural-main-20261008.json).
The five upstream installer boundary cases are now independently audited
using separately identified new raw logs; the missing historical raw path
is recorded, not relabeled as consumed.

Acceptance evidence now covers the scoped implementation/parity tests,
three successful PR and push cohorts, five warm samples on all platforms,
native jobs, unchanged requiredness and the installer failure boundaries.
The final documentation submission must pass its own Required Quality
Aggregate bound to its actual commit. Historical observations do not replace
that status. No additional natural runs or optional sparse-checkout experiment
are needed to restate the measured boundaries.

Push-only coverage and five-sample native baselines remain push-only. No trigger,
platform, sample count, threshold, required job, aggregate condition or failure
path changes. Base/head measurement collection stays independent. The optional
aggregate sparse-checkout experiment is not adopted: its dependency closure and
measured benefit have not been established, so full checkout remains intact.
