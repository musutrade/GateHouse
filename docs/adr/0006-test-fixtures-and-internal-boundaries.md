# ADR 0006: Test Fixtures and Internal Workflow Boundaries

## Status

**Accepted** - 2026-08-27

## Context

Harness Gate's test suite has grown several copies of temporary workspace,
Git repository, and preset setup. Some unit tests manually construct
timestamp-based directories and delete them at the end of the test, while the
process-level tests use a separate context object. At the same time, Git path
decoding, scope selection, and audit ignore-file traversal are internal
boundaries whose ownership is not clear from the implementation.

## Decision

Use a crate-private `cfg(test)` workspace fixture for in-process tests and keep
the existing integration-test `TestContext` as the process-level fixture. Both
fixtures own temporary workspace lifecycle and concise setup helpers; they do
not share mutable state or cache repositories between tests.

Keep NUL-delimited Git output parsing in `utils/git`. Keep scope command
selection, path de-duplication, unmatched-path policy, and report generation in
`scope`. Audit owns its ignore-file traversal configuration. Mark shared
filesystem and Git helpers `pub(crate)` to make their internal-only status
explicit.

## Consequences

- Unit tests gain automatic cleanup when a test panics and use less repeated
  setup code.
- Integration tests retain end-to-end process isolation.
- Internal ownership is visible in both code documentation and Rust visibility.
- No CLI, report, TOML schema, Git command, or error-code behavior changes.

## Alternatives Considered

- A persistent shared fixture: rejected because it risks cross-test state and
  changes test isolation.
- A public test support crate: rejected because Harness Gate is a binary crate
  and has no external consumers for these helpers.
- A generic Git abstraction: rejected because the current narrow helper API is
  sufficient and a broader wrapper would obscure command-specific behavior.

## Related

- [ADR-0005](0005-phase-2-optimization-strategy.md)
- [OpenSpec change: improve-test-fixtures-and-boundaries](../../openspec/changes/improve-test-fixtures-and-boundaries/proposal.md)
