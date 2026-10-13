# Adapter Request Integrity and Evidence Budgets

This follow-up closes the next-cycle adapter and evidence-boundary findings
R-06, R-07 (documentation/reader contract), R-09, and R-11.

Published invocation evidence is capped at 256 MiB overall and 16 MiB per
file; process streams and adapter artifacts have their own lower, explicit
budgets.

- ADR: [0037-adapter-request-integrity-and-evidence-budgets](../../../docs/adr/0037-adapter-request-integrity-and-evidence-budgets.md)
- Status: implemented in the priority follow-up branch; CI evidence is recorded
  in the pull request that merges this change.
