# Design: Code-Review Follow-ups

## Scope matcher cache

`CompiledScopeRules` owns one `GlobSet` per configured scope rule. `Project`
constructs it immediately after loading and validation-ready configuration, and
scope detection passes the cached matchers to classification. The existing
`FlowConfig::classify_paths` helper remains available for isolated callers and
compiles a temporary set, so its behavior and error contract stay unchanged.

## Skipped-step evidence

`VerificationReport` gains an optional `skipped_steps` array containing the
stable node ID, label, and blocking reason. Successful reports omit the field
through `skip_serializing_if`, preserving current snapshots. Markdown emits a
`SKIPPED` line and JUnit emits a `<skipped>` testcase; failures continue to count
only dispatched failed steps.

## Platform baseline

The CI `quality-baseline` job becomes a fail-fast-disabled matrix over Ubuntu,
macOS, and Windows. The fixture selects the Python interpreter that launched
the runner and uses an exclusive-create lock instead of POSIX-only `fcntl`.
Artifacts include the platform in their names. The checked-in Linux record
remains the canonical baseline; other targets are reviewed as separate series.
