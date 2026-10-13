## 1. Test Support

- [x] 1.1 Add a `cfg(test)` workspace fixture with automatic temporary-directory cleanup and verify its unit consumers compile (Priority: P1, Effort: S) - `TestWorkspace` uses `tempfile::TempDir` and test targets compile.
- [x] 1.2 Extend integration `TestContext` with preset and Git setup helpers, then migrate repeated CLI/integration setup calls and verify the integration tests pass (Priority: P1, Effort: M) - `init_preset`/`init_project` are used by integration tests; nextest passes.
- [x] 1.3 Migrate audit, secrets, verification, and project unit tests from manual timestamped paths to the shared fixture and verify their tests pass (Priority: P1, Effort: M) - Also migrated doctor and preset tests; nextest passes.

## 2. Internal Boundaries

- [x] 2.1 Make internal filesystem and Git helpers crate-visible and verify all callers compile without public API changes (Priority: P1, Effort: S) - `utils::fs` and `utils::git` helpers are `pub(crate)`.
- [x] 2.2 Extract and test NUL-delimited Git path parsing, including embedded newlines and non-UTF-8 failure behavior (Priority: P1, Effort: S) - Dedicated unit tests cover both cases.
- [x] 2.3 Document the Git, scope, and audit ignore-file ownership boundaries in code and verify rustdoc builds (Priority: P2, Effort: S) - Inline boundary docs added; `cargo doc --no-deps` passes.

## 3. Verification

- [x] 3.1 Run `TMPDIR=/tmp cargo nextest run --manifest-path tools/harness-gate/Cargo.toml` and verify all tests pass (Priority: P0, Effort: S) - 81 tests passed.
- [x] 3.2 Run format, Clippy, and rustdoc checks and verify no warnings or documentation failures (Priority: P0, Effort: S) - Format, Clippy with `-D warnings`, and rustdoc all pass.
