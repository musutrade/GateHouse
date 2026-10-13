# Proposal: Confined Report-Template Rendering

**Status:** Proposed
**Date:** 2026-08-29
**Change type:** Report rendering, schema, and filesystem-safety follow-up

## Scope

This proposal is the follow-up required before report-template rendering is
treated as a stable public configuration contract. The existing configuration
safety change validates repository-contained template inputs; this proposal
defines the rendering behavior, migration surface, and compatibility evidence
before any further template fields are accepted.

## Decisions Required

1. **Rendering mode and defaults.** Decide whether rendering is opt-in per
   workflow, which output formats are enabled, the default behavior when the
   section is absent, and how a renderer failure affects the existing JSON and
   Markdown reports and public error codes.
2. **Schema diff.** Version and review the exact `[report_templates]` fields,
   allowed extensions, output names, include/inheritance settings, and any
   future asset declarations. Existing `paths.*` inputs must not be widened
   implicitly.
3. **Compatibility policy.** Define behavior for existing v2 files, v1
   migration output, disabled rendering, malformed templates, and partial
   report publication. Preserve the established report shape unless a separate
   snapshot decision approves a change.
4. **Loader confinement.** Keep template, include, inheritance, and asset
   resolution under one canonical repository-contained template root. Reject
   symlink escapes, report-root overlap, non-regular files, and platform path
   prefixes before rendering.
5. **Validation evidence.** Add schema, migration, unit, integration, and
   cross-platform tests for the decisions above. Tests must prove that
   renderer failures retain base reports and do not disclose template or
   environment contents.

## Non-goals

- No new template fields are authorized by this proposal alone.
- No arbitrary filesystem reads, remote templates, or implicit path repair.
- No change to the existing audit or project-selection configuration rules.

## Related Records

- [Configuration safety OpenSpec](../configuration-safety-diagnostics/proposal.md)
- [ADR-0026](../../../docs/adr/0026-configuration-safety-diagnostics.md)
- [Phase 1 quality gates](../phase-1-quality-baseline-gates/proposal.md)
