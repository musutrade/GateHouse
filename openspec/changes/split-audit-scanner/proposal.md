# Proposal: Split the Audit Scanner

## Why

The scanner mixes path safety, lexical parsing, file filtering, and two scan
workflows in one module. Separating these responsibilities improves reviewability
while retaining the existing audit contract.

## What Changes

- Define private path, lexical, filter, hard-rule, and architecture modules.
- Preserve current `scanner` exports for callers within `audit`.
- Do not change traversal, filtering, comments, regexes, reports, or errors.

## Non-goals

- No audit rule/schema changes.
- No changes to comments/strings semantics, parallelism, output, or dependencies.
- No production code change in this design PR.

## Success Metrics

- This documentation PR contains no production-source changes.
- The implementation PR will pass existing tests and preserve scanner behavior.
