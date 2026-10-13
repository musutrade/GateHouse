## Context

See [proposal.md](proposal.md). Unit tests currently create timestamped
directories and remove them manually, while process-level tests have a separate
`TestContext`. Git execution is centralized in `utils/git`, but the path format
and scope-selection boundaries are not explicit in code comments.

## Goals / Non-Goals

**Goals:**

- Use a `#[cfg(test)]` workspace fixture backed by `tempfile::TempDir` for
  in-process tests.
- Keep external-process test setup in `tests/common`, adding only ergonomic
  operations it owns.
- State boundaries in code: Git output decoding belongs in `utils/git`; scope
  owns command selection, de-duplication, and config classification.

**Non-Goals:**

- Do not create a reusable public test library.
- Do not alter command arguments, Git timeouts, error codes, or output files.

## Decisions

1. Add one crate-private, test-only workspace helper.
   `TempDir` gives automatic cleanup even on assertion failure. It replaces
   hand-built timestamp paths in selected unit tests. A shared persistent
   fixture was rejected because test isolation matters more than avoiding a few
   Git initializations.

2. Keep `tests/common::TestContext` separate.
   Integration tests execute the compiled binary and require its own command
   helper. It gains setup methods instead of depending on a crate-internal test
   module, preserving the integration boundary.

3. Keep Git parsing in `utils/git` and scope policy in `scope`.
   A private parser converts NUL-delimited Git output to paths; `scope::detect`
   selects commands and interprets unmatched files. Extracting a generic Git
   abstraction would add indirection without another caller.

4. Use `pub(crate)` for shared internal helpers.
   The project is a binary crate, and these helpers are not external API. The
   narrower visibility documents that only internal workflows may call them.

## Risks / Trade-offs

- [Fixture migration hides per-test intent] -> Keep explicit test names and
  fixture setup calls at the test site.
- [Test-only helper grows into production infrastructure] -> Compile it only
  under `cfg(test)` and keep its API focused on filesystem and Git setup.
- [NUL parsing changes behavior] -> Add direct parser tests for delimiter and
  UTF-8 error cases; leave command construction untouched.

## Migration Plan

1. Add the fixture and parser tests.
2. Migrate affected tests and document module boundaries.
3. Run the full validation suite. Revert the isolated refactoring commit if a
   failure occurs; no data or configuration migration is required.
