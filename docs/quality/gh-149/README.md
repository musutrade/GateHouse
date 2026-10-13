# GH-149: Rust cross-component validation and project reporting

This slice implements OpenSpec `consolidate-generic-quality-core-into-rust` tasks
4.1–4.2 in the independent `harness-gate-quality-core` candidate library. The
predecessor GH-148 is complete: PR #155 merged, with Required Quality Aggregate
and the applicable required checks successful on
`19b1e338877283a3855716762b58ae7b15d8c37b`.

`cross_component` owns relationship lookup, provider-owned contract subject
selection and provenance validation. Policy evaluation invokes it directly after
normalized evidence and artifact validation, replacing the temporary callback.
Participant identity, contract/source digest, metric artifact references, pinned
baseline commit and series, consumer expectations and generated-client drift
must agree before a supported metric reaches the existing typed comparison.
These checks consume generic contract fields; no tool parser or language-specific
rule enters the library.

`project_report::report` preserves the complete policy result and gate records,
including raw evidence links, under `harness-project-report/v1`. Ordered gate IDs
feed component, subject, policy, status and relationship indexes. Provider and
consumer share cross-component references without duplicating the project gate
table or its blockers. Component, local and cross-component aggregates reuse
policy-owned requiredness; empty groups remain `not_applicable`.

The test-only oracle replays the 33 hash-verified frozen cases, then captures the
existing policy, ratchet and project-report reference tests over retained bytes.
Complete Python and Rust policy results and reports are compared, including every
stable field, blocker, gate-table entry, index and evidence link. The Rust report
is checked both from Python policy results and from independently evaluated Rust
results. A dedicated end-to-end assertion proves that local API/frontend gates
pass while a breaking contract fails both participants and the project with the
same provenance. Negative cases include missing/mismatched bindings, stale base
commit, incompatible series, missing consumer/client evidence, contradictory
drift, missing/unavailable/tampered evidence, target mismatch and multiple
relationships sharing a contract subject. Empty reports and null subject indexes
have additional reference comparisons.

Local validation passed 325 Rust tests and 289 Python tests. The final oracle
corpus contains 826 comparisons, including 133 complete reports, with zero
unexplained mismatches. Formatting, Clippy, documentation consistency and strict
OpenSpec validation passed.

Actual command results and complete logs are retained in [validation.json](https://github.com/musutrade/Harness-Gate/blob/d9a932b23dcfea709175285b594ca200450a9971/docs/quality/gh-149/validation.json)
and [validation.tar.gz](https://github.com/musutrade/Harness-Gate/blob/d9a932b23dcfea709175285b594ca200450a9971/docs/quality/gh-149/validation.tar.gz). The only normalized diagnostic
variation is the previously classified Rust/Python missing-file wording: the
same source/artifact path and missing-file failure must be present. No semantic
fields are dropped. Cargo uses workspace-local cache/build directories because
the shared cache rejects writes; the original error is retained.

The checkout has no `.harness-gate/flow.toml` declaring a `ci` profile, so
`harness-gate config check` and `harness-gate verify --profile ci --all` are not
applicable, not passed. Python is used only as a test oracle for the candidate.
The CLI has no dependency on this library. Replay entry points, hosted differential
acceptance, authority transfer and Python disposition remain tasks 5–7 under
[ADR-0040](../../adr/0040-language-agnostic-evidence-policy.md). Required hosted CI
on this issue's final pushed SHA is pending for the controller; this evidence does
not accept the full proposal.
