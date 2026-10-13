# Proposal: Strict Standard Result XML

**Status:** Implemented
**Date:** 2026-09-02

JUnit and TRX result ingestion previously checked tag balance but did not
enforce the XML document boundary. Multiple roots, a missing root, or an
unrelated root containing countable element names could therefore contribute a
test count. This change makes those inputs fail closed before the minimum count
is evaluated.

## Goals

- Require exactly one XML root for JUnit and TRX result files.
- Accept only `testsuite` or `testsuites` for JUnit and `TestRun` for TRX.
- Reject missing roots and non-whitespace content outside the root.
- Preserve standard namespace-prefixed result files by comparing local names.
- Cover accepted and rejected shapes with focused regression tests.

## Non-goals

- Do not validate every attribute or child ordering rule from every JUnit
  producer dialect or the full Visual Studio TRX schema.
- Do not change regex or JSON parser behavior.
- Do not change result minimums or the public machine-result schema.

## Success Metrics

| Boundary | Success criterion |
| --- | --- |
| Document | Empty, multi-root, and trailing-content XML returns a parse failure. |
| Format | A wrong JUnit or TRX root returns a parse failure. |
| Compatibility | Existing direct/nested JUnit suites, TRX results, and namespace-prefixed names retain their counts. |
| Regression | Focused parser tests and the full locked test suite pass. |

## Risk Assessment

**Risk: Low.** The change intentionally rejects documents that are not valid
JUnit/TRX document shapes. Some permissive callers may have relied on XML
fragments; they can wrap results in a supported root or temporarily select the
existing regex compatibility parser.

## Related Records

- [ADR-0034: Fail Closed at Trust Boundaries](../../../docs/adr/0034-fail-closed-trust-boundaries.md)
- [Review follow-up R-12](../../../docs/review-followups-2026-08-31.md#3-p2p3-%E5%90%8E%E7%BB%AD%E8%B4%A8%E9%87%8F%E5%80%BA)
