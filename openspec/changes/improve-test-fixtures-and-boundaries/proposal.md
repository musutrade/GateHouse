## Why

The test suite repeats temporary-directory lifecycle, preset initialization,
and Git setup in unit and integration tests. The scope and Git helpers also
carry safety-critical behavior without documenting which module owns each
responsibility. Consolidating these details reduces maintenance cost while
keeping the CLI contract unchanged.

## What Changes

- Add a test-only workspace fixture for in-process unit tests and extend the
  integration-test context with common preset and Git setup operations.
- Migrate representative audit, secrets, verification, project, CLI, and
  integration tests away from hand-managed temporary paths and Git commands.
- Document the NUL-delimited Git path boundary, scope-selection rules, and
  audit ignore-file traversal behavior.
- Mark internal file and Git helpers as crate-visible rather than presenting
  them as public APIs.

## Goals

- Preserve all CLI arguments, output, configuration schemas, and reports.
- Make temporary test workspace cleanup deterministic.
- Give future maintainers one clear owner for Git command handling and scope
  selection behavior.

## Non-goals

- Changing Git commands, scope-selection semantics, or audit ignore rules.
- Sharing fixture state across tests or changing test parallelism.
- Adding production dependencies or a public library API.

## Success Metrics

- The affected tests use the shared fixture rather than manual timestamped
  temporary directories and cleanup.
- Tests cover NUL path parsing and preserve its failure behavior for non-UTF-8
  paths.
- `cargo nextest run`, Clippy, format, and docs checks pass.

## Capabilities

### New Capabilities

None. This is an internal refactoring change with no user-visible behavior.

### Modified Capabilities

None.

## Impact

- `tools/harness-gate/src` test support, scope, Git utility, and audit modules
- `tools/harness-gate/tests` common, CLI, and integration test fixtures
- ADR-0006 and the existing Phase 2 OpenSpec follow-up record

Risk: Low. The change is test support, documentation, and visibility only;
existing tests continue to exercise the same production behavior.
