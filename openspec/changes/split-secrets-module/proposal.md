# Proposal: Split the Secret Scanner Module

## Why

`tools/harness-gate/src/secrets.rs` grew to 807 lines and combined secret
configuration parsing, rule compilation, matching heuristics, Git/filesystem
scanning, reports, and tests. The mixed responsibilities increase review cost
for security-sensitive changes.

## What Changes

- Move secret configuration models and rule compilation into
  `secrets/config.rs`.
- Move matching logic and secret-value heuristics into `secrets/matcher.rs`.
- Keep scan orchestration, report serialization, `SecretMode`, and
  `SecretsError` in `secrets/mod.rs`.
- Move unit tests into `secrets/tests.rs`.
- Keep existing `crate::secrets` exports and behavior unchanged.

## Goals

- Give configuration, matching, and scanning clear ownership boundaries.
- Keep implementation files substantially smaller than the original module.
- Preserve scan results, report fields, error codes, and configuration
  compatibility.
- Keep extracted modules private and narrowly coupled.

## Non-goals

- Changing secret rules, placeholder policies, or scan-mode semantics.
- Changing report filenames or JSON fields.
- Replacing regex, URL, Git, or filesystem implementations.
- Adding production dependencies or a public library API.

## Success Metrics

- The parent module contains only scan orchestration and stable boundary types.
- Existing unit, CLI, and integration tests pass without behavior changes.
- Format, Clippy with `-D warnings`, rustdoc, and strict OpenSpec validation
  pass.
- No public symbols, error strings, or generated report shapes change.

## Risk Assessment

Low to medium. The extraction stays within a binary crate; the primary risks
are private visibility mistakes and accidental changes to serde defaults or
matching heuristics. Existing tests and static checks provide fast feedback,
and reverting the refactoring commit restores the previous layout.
