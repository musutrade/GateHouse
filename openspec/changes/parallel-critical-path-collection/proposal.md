# Reuse critical-path builds and overlap isolated tests

## Goals

Build the selected revision once per collection, reuse nextest metadata, and run
up to two exact critical-path tests concurrently by default. Keep per-test profiles
isolated and serialize LLVM report exports. Relevant engineering-policy semantics
are unchanged: source-v2 identities, mandatory rows, thresholds, no retries masking
failures, and fail-closed evidence validation remain in force.

## Non-goals

No CI job topology, risk series, base/head collection, baseline, or release changes.
No cross-run cache of measured coverage. No production plugin packaging changes.

## Success Metrics

One instrumented build per collection; identical reviewed probes pass for all 12
Linux rows; concurrency and contamination failure controls pass. Record fresh local
before/after timing without treating local results as hosted CI measurements.

## Risk

Medium: shared build state could contaminate profiles. Mitigate with a fresh owned
build root, nextest binary reuse, private per-run LLVM_PROFILE_FILE including child
processes, profile-only clean and serialized exports. Keep command logs on failure.
See docs/engineering-policy.md and docs/quality/critical-paths.md. Rollback is a code
revert to serial collection, with the same policy and evidence rule.
