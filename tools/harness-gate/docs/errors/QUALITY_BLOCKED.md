## Why this fails

A required quality decision failed, or its inputs could not be trusted. This
code alone does not distinguish a measured policy failure from missing, stale
or malformed evidence. Inspect the normalized findings and evidence validation
errors before choosing a repair.

## Diagnose and repair

1. Locate the failing rule and subject in the quality report.
2. If evidence is invalid, repair its producer and generate fresh evidence for
   the same candidate. Never rewrite a manifest to relabel old measurements.
3. If a measured threshold failed, inspect the specific function or component.
   Add tests for missing behavior or simplify its implementation, then measure
   again with the unchanged policy.

## Example

A function with a high complexity score and uncovered error branches needs
behavioral tests or a simpler implementation. Useful cases may include invalid
input, a missing dependency and a failed write. Passing a new unit test alone
does not prove the gate passes: rerun the required quality measurement and
inspect the resulting decision. Do not raise thresholds to hide the failure.
