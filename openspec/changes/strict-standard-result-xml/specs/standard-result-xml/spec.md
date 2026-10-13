# Standard Result XML

## ADDED Requirements

### Requirement: Standard XML results have one recognized root

The JUnit parser SHALL accept exactly one root whose local name is `testsuite`
or `testsuites`. The TRX parser SHALL accept exactly one root whose local name
is `TestRun`. A missing, unrelated, or additional root SHALL produce
`RESULT_PARSE_FAILURE` before a minimum test count is evaluated.

#### Scenario: Multiple JUnit roots are supplied

- **WHEN** a JUnit result contains two sibling `testsuite` elements
- **THEN** parsing fails and the result cannot satisfy the gate minimum

#### Scenario: A namespace-prefixed standard result is supplied

- **WHEN** the root and countable elements use qualified names with recognized
  local names
- **THEN** the parser accepts the document and counts those result elements

### Requirement: Content is confined to the XML root

The JUnit and TRX parsers SHALL reject non-whitespace text and all CDATA
outside the single root. XML declarations, comments, processing instructions,
and whitespace MAY surround the root; a declaration or doctype after the root
is invalid.

#### Scenario: Text follows a closed result root

- **WHEN** non-whitespace text occurs after the result root
- **THEN** parsing fails with no accepted test count

## Implementation Plan

| Phase | Timeline | Scope | Exit evidence |
| --- | --- | --- | --- |
| 1 | Day 1 | Add root/document state to the streaming parser | Focused parser tests pass |
| 2 | Day 1 | Document compatibility and fail-closed cases | Strict OpenSpec validation passes |
| 3 | PR | Run full repository verification | Required CI aggregate is green |

## Success Criteria

- Valid direct and nested JUnit suites and TRX `TestRun` documents retain their
  expected counts.
- Empty, wrong-root, multi-root, unclosed, and trailing-content inputs fail.
- The implementation remains streaming and adds no new dependency.

## Example

```xml
<testsuites>
  <testsuite name="unit">
    <testcase name="passes"/>
  </testsuite>
</testsuites>
```

## Alternatives Considered

- Full XSD validation was rejected because JUnit dialects do not share one
  authoritative schema and TRX schema validation would add disproportionate
  dependency and compatibility cost.
- Keeping fragment parsing was rejected because it lets unrelated or
  concatenated XML contribute a passing count.

## Rollback Plan

Revert this change if a recognized producer emits a valid but unsupported root
shape, preserve its fixture, and revise the explicit allowlist. Do not restore
unrestricted multi-root parsing.
