# Proposal: Close Code-Review Follow-ups

**Status:** Implemented
**Date:** 2026-08-30
**Change type:** Verification observability, scope performance, and quality evidence

## Why

The 2026-08-30 code-review note predates the merged parallel scheduler and
quality baseline work. It also identifies two remaining usability gaps: scope
globsets are rebuilt for every classification, and dependency-blocked steps are
not visible in verification reports. The performance baseline currently runs
only on Linux even though the release supports three operating systems.

## What Changes

- Compile scope globsets once during project discovery and reuse them for the
  lifetime of a `Project`.
- Add optional skipped-step evidence to JSON, Markdown, and JUnit reports while
  preserving the existing successful-report shape.
- Run the performance/size baseline on Ubuntu, macOS, and Windows with a
  portable benchmark fixture; keep the accepted Linux record as the canonical
  numeric series and retain other platforms as independent series.
- Correct the code-review document's version, implementation status, and
  measured-size claims.

## Compatibility

- Serial execution remains the default and all existing step results retain
  their fields and ordering.
- `skipped_steps` is omitted from successful JSON reports and only appears when
  dependency-blocked nodes exist.
- Baseline comparisons never mix target triples or toolchains.
- No user project files or existing release artifacts are changed.
