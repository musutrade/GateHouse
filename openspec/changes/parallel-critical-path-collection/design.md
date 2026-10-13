# Collection design

The engineering-policy semantics are unchanged. The source-v2 evaluator remains
unchanged; this modifies execution scheduling and build reuse, not measurement
requiredness, thresholds, probe selection, baseline rules or decision authority.

1. Reserve the existing collection lock and check source/commit identity.
2. Create a unique owned build directory. Obtain the pinned cargo-llvm-cov
   instrumentation environment as assignments, parse it as data without a shell,
   and build/list nextest binaries once. Retain cargo and binary metadata.
3. Run exact nextest filters using the build metadata with a bounded executor.
   Each test gets its own fresh `%p-%m.profraw` directory inherited by child CLIs.
   Test threads and retries are fixed at one and zero respectively. Build-time
   profiles remain outside these private directories.
4. After all tests finish, serialize profile-only cleanup, staging and report
   export. No reporting command overlaps another report or a test. Preserve the
   original profiles and command logs. The existing clean_exit field records
   actual profile cleanup; no exit status is synthesized.
5. Verify source identity again, discard successful temporary build state and
   publish the completed bundle. Failed commands retain diagnostics and cannot
   publish a success bundle. Tests can finish out of order; published rows remain
   in inventory order.

## Alternatives and rollback

Independent CI jobs would duplicate compilation and require artifact topology
changes. Parallel cargo-llvm-cov commands on the existing shared directory would
race cleanup and profile merging. Direct LLVM export would add another object
selection implementation. This change instead reuses nextest's build metadata
and cargo-llvm-cov's existing export logic. `--jobs 1` provides serial test
execution for diagnostics. Reverting this change restores the old collector
without changing policy or requiring baseline adoption.

## Validation

Unit controls cover overlapping tests, build-only profile exclusion, identical
profile names in different tests, failed/missing evidence, source changes, locking
and cleanup. Run real Linux collection against committed snapshots for all 12
reviewed rows, comparing probe counters and statuses with the old implementation.
Timing is local wall time, not a hosted CI speed guarantee.
