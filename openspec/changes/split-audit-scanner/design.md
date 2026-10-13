# Design: Split the Audit Scanner

The parent module will retain the internal scanner interface used by the audit
runner. It will delegate repository path resolution to `paths`, lexical source
analysis to `lexical`, reusable path eligibility to `filter`, and each rule
family to its dedicated scan module.

Implementation must preserve the exact order of filtering: regular files,
extensions, configured exclusions, path regex exclusions, then allowlists.
It must retain NUL-safe and UTF-8 behavior, configured comment syntax, nested
block comments, line deduplication, and relative violation paths.

## Rollback

Revert the future implementation commit; this design introduces no runtime
behavior or configuration changes.
