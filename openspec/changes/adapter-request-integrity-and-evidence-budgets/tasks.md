# Tasks: Adapter Request Integrity and Evidence Budgets

- [x] 1.1 Version the adapter request protocol and sign the canonical complete
  request envelope.
- [x] 1.2 Add validity-window, nonce, and policy-scoped replay checks.
- [x] 1.3 Validate environment keys/values and reserved host metadata before
  process creation.
- [x] 2.1 Add bounded shared stdout/stderr readers, truncation failures,
  artifact byte budgets, request-size limits, and reader deadlines.
- [x] 2.2 Apply shared redaction to audit, parsed-log, verification, webhook,
  and error output paths; strip dependency URL userinfo from generated SBOMs.
- [x] 2.3 Add tamper, replay, expiry, invalid-environment, output-limit, and
  redaction regression tests.
- [x] 3.1 Align README, configuration docs, ADR-0033, and adapter OpenSpec with
  the protocol and non-sandbox guarantees.

## Evidence Review

Focused adapter, process, audit, and full crate tests cover the success and
fail-closed paths. The pull request records formatter, Clippy, OpenSpec, and
documentation-consistency results before merge.
