# Critical-path collection validation

The engineering-policy semantics are unchanged. Mandatory rows, source-v2 rules,
probe selection, thresholds and fail-closed checks remain intact.

On 2026-09-13, both original and updated collectors ran against committed isolated
worktrees with identical Rust compilation inputs and inventory, using the same
local Rust/nextest/LLVM tools and `CARGO_BUILD_JOBS=4`.

| Measurement | Original | Updated |
| --- | ---: | ---: |
| Project compilations | 12 | 1 |
| Applicable Linux paths passed | 12/12 | 12/12 |
| Wall time | 446.6 s (whole CLI) | 111.8 s (collection including cleanup) |

All reviewed probe coordinates and hit counts, exact test identities, row statuses
and the final summary match. The updated collector uses two test workers and
serialized profile exports. Its build took 93.5 s and tests
6.0 s. The successful build directory was confirmed removed.
A preliminary run before adding successful-build cleanup took 80.9 s.

These are indicative shared-host observations with concurrent workloads, not a
controlled hosted-CI benchmark or a promised speedup for the whole quality job.
The original stopwatch includes final evaluation; the updated collector stopwatch
ends at completed collection. macOS/Windows and hosted CI have not been exercised.

Validation: all 352 quality-script tests passed; the final focused collector and
evidence run passed 23 tests; documentation consistency and `git diff --check`
passed. Unit failure controls include shared-profile contamination, missing profiles,
failed builds/tests/cleanup/exports, source changes, overlapping collection locks,
and bounded retention of failed build trees.

See [machine-readable results](validation.json) for tool versions, exact counters,
source hash, timings and local raw evidence locations. Reproduce after committing
collector source changes:

```sh
CARGO_BUILD_JOBS=4 python3 tools/quality/critical_paths.py --collect --jobs 2 \
  --evidence target/quality/critical-path-runs/bundle.json \
  --output target/quality/critical-paths.json
```

Use `--jobs 1` to diagnose serial test execution while preserving build reuse.
