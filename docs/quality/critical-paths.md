# Critical-path source evidence

`tools/quality/critical_paths.toml` uses rule `critical-path-source-v2`. Each stable
ID identifies a Rust function, exact source range and file SHA-256, an exact nextest
binary/test identity, its test function and assertions, executable line/column
probes, and a reviewed observable. Platform exclusions have a reason, owner and
review date. The independent `critical_paths_policy.json` fixes the six mandatory
IDs and their applicability. Removing one fails before computing the denominator.

Commit source changes, then collect and evaluate:

```sh
python3 tools/quality/critical_paths.py --collect \
  --evidence target/quality/critical-path-runs/bundle.json \
  --output target/quality/critical-paths.json
```

The collector requires cargo-nextest, cargo-llvm-cov (CI pins 0.9.0), and Rust's
llvm-tools-preview component. The collector builds instrumented test binaries once
in a fresh `target/critical-path-build/<collection-id>` directory, then uses nextest
build metadata to reuse those binaries. Two exact tests run concurrently by default;
`--jobs 1` selects serial execution and `--jobs N` accepts 1–8 workers. Each test and
its child CLI processes write profiles to a unique run directory. No test cleans or
rebuilds the shared binaries. Retries are disabled for exact single-test evidence.

After tests finish, report exports run serially. Each export cleans only the shared
report directory's raw profiles, stages one test's profiles, and exports LLVM JSON.
The originals remain in that test's run directory. Build/list-time counters and
other tests' counters are excluded. This preserves the existing source-v2 rule,
mandatory rows, thresholds and fail-closed engineering-policy semantics.

An atomic collection lock rejects overlapping collectors. A crash leaves the lock
for inspection before manual removal. Every command, exit status, test event stream
and coverage export is retained, including failed runs. Only a completed collection
writes the bundle declaration. `clean_exit` now describes profile-only report
preparation, not a rebuild. Build, test and total collection timings are included
in the bundle; individual commands also record elapsed seconds. The fresh build
directory is removed after successful collection; failed builds remain available for
diagnosis, and the next collection reclaims the oldest failed build trees, keeping
the newest three. Build binaries are not included in the evidence bundle.

The bundle binds inventory, all crate source/test/build inputs, host target,
commit, rule, tool versions and artifact hashes. Local source bytes must match the
commit. A row cannot reuse another run's artifacts. Missing, modified, stale,
mixed-target, mixed-commit, skipped, cancelled or failed evidence fails closed.
Names-only lists and aggregate module coverage are no longer accepted. CI's
existing matrix step invokes this collector and uploads its raw artifacts.

LLVM function envelopes must match the reviewed source function. Each probe needs
a positive counter in its most specific executable region; an outer function hit
cannot substitute for an unexecuted failure branch. Changes to function locations,
file contents or observable assertions require an explicit inventory update and
fresh evidence. Inspect the source and LLVM regions when updating columns: Rust
keywords such as `break` and `let` may have no executable region of their own.

Passing assertions bind the observed behavior to the isolated source execution;
the tool does not infer assertion semantics. Review assertion changes together with
the test and helper source. The JSON fixture runs the actual CLI against unrelated,
numeric and ambiguous successful-command output, then checks failure status,
`RESULT_PARSE_FAILURE`, and incomplete parser evidence. Report tampering and lease
ownership fixtures inject actual boundary failures. Fake Docker/Podman ownership
evidence makes no real-container or daemon claim. The process-tree fixture checks
for a delayed descendant marker; it is not proof of complete descendant containment.

Every applicable mandatory row must pass even if the overall percentage exceeds
95%. At least 95% of all applicable rows must pass. Unix-only cancellation and JSON
CLI fixtures are reviewed exclusions on Windows; Linux and macOS require them.
No platform becomes exempt merely because a runtime dependency is unavailable.

This implements OpenSpec `strict-json-results-and-risk-based-quality-gates` tasks
7.1–7.4 and operationalizes the failure boundaries in
[ADR 0034](../adr/0034-fail-closed-trust-boundaries.md) and
[ADR 0038](../adr/0038-post-remediation-hardening.md). Broader quality aggregation
and baseline adoption remain tasks 8.x.

Local before/after timing, tool versions and the review of the parallel
collection change are in the
[collection benchmark](../benchmarks/critical-path-collection/README.md).
