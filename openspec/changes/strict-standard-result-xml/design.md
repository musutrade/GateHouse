# Design: Strict Standard Result XML

The streaming XML reader retains the existing bounded-memory tag stack. It now
tracks whether a root has been seen and closed. A start or empty element at
depth zero must have a format-specific allowed local name; another depth-zero
element is rejected as a second root. Non-whitespace text or CDATA outside the
root is also rejected, and EOF without a root remains a parse failure.

Qualified names are retained on the stack for exact start/end matching, while
root and countable element classification uses the local name. This accepts
ordinary namespace-prefixed JUnit/TRX documents without weakening the single
root rule.

## Compatibility

Valid JUnit documents rooted at `testsuite` or `testsuites` and valid TRX
documents rooted at `TestRun` keep their current counts. XML declarations,
comments, processing instructions, and surrounding whitespace remain accepted.
Arbitrary XML fragments and unrelated roots are no longer treated as standard
test-result documents.

## Rollback

Revert the parser and documentation change through protected `main`. Callers
that need fragment matching can select the regex parser without weakening the
JUnit/TRX format contract.
