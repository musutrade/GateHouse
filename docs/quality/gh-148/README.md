# GH-148: Rust policy and ratchet semantics

Scope: tasks 3.1–3.2 of [the OpenSpec change](../../../openspec/changes/consolidate-generic-quality-core-into-rust/tasks.md).
The predecessor GH-147 merged as PR #154 with required CI complete before this work.

The candidate [policy module](../../../tools/harness-gate/quality-core/policy.rs)
validates policy, compares all seven typed values without floating-point rounding,
selects scopes, emits `GateResult` values and aggregates required gates. Requiredness
comes exclusively from policy. Missing required results block; measurement errors
take precedence over failures, which take precedence over other blocking states.
Collectors continue to supply evidence and capability states, never final decisions.

The [ratchet module](../../../tools/harness-gate/quality-core/ratchet.rs) validates
base provenance, measurement-series compatibility and explicit subject mappings.
Exact identities and one-to-one modify/rename/move mappings inherit history; splits
remain new and cannot duplicate legacy-debt permission. Absolute compliance,
trend, remaining debt and regression are recorded separately. Exception metadata
can document debt but cannot waive a quality failure; invalid, missing, expired,
duplicate or unknown exception metadata makes the combined aggregate a measurement
error while preserving the original quality aggregate and gate results.

The [test-only oracle](../../../tools/harness-gate/quality-core/tests/policy_reference.py)
first verifies the immutable corpus hashes and reproduces all 33 frozen Python
policy outputs. It also captures calls from the 19 existing Python policy/ratchet
tests and adds deterministic arithmetic, scope and policy boundary cases. The
391 differential cases comprise 120 evaluations, 181 typed comparisons, 36 ratchet
decisions, 23 aggregates, 19 policy validations and 12 selections. Comparisons use
integers beyond machine ranges, very small exact decimals and all six operators.
Every result field is compared, including blockers, exceptions, lineage, debt
ledger and remediation context. The only normalized differences are the already
classified Rust/Python OS diagnostics for the two frozen missing-source cases:
both must identify the same artifact/source path and missing-file failure. These
are diagnostic formatting differences, not semantic mismatches.

Contract fixtures supply the retained Python contract validator's outcomes through
an explicit callback. This isolates the policy behavior in tasks 3.1–3.2 from
cross-component provenance validation in task 4.1. The production candidate fails
closed when that callback is absent, and the differential test checks this boundary.
The library has no Python runtime dependency and no CLI dependency. Project reports,
replay commands, CI integration and authority transfer remain tasks 4–7 under
[ADR-0040](../../adr/0040-language-agnostic-evidence-policy.md). Existing required
workflow definitions and the CLI measurement/coverage boundary are unchanged.

Actual local results and complete logs are retained in [validation.json](https://github.com/musutrade/Harness-Gate/blob/d9a932b23dcfea709175285b594ca200450a9971/docs/quality/gh-148/validation.json)
and [validation.tar.gz](https://github.com/musutrade/Harness-Gate/blob/d9a932b23dcfea709175285b594ca200450a9971/docs/quality/gh-148/validation.tar.gz). Cargo uses workspace-local
`target/gh-148/cargo-home` and `target/gh-148/build` because the shared Cargo cache
and target directory are read-only in this environment. Python is needed for
the differential tests only. This is bounded compatibility evidence for the
frozen corpus, not acceptance of the entire migration or ecosystem certification.
Exception timestamps use Rust RFC 3339 parsing; compatibility evidence covers the
frozen timestamp forms, not every additional spelling accepted by Python ISO parsing.

This checkout has no `.harness-gate/flow.toml` declaring a `ci` profile;
`harness-gate config check` and `harness-gate verify --profile ci --all` are not
applicable, not passed. Hosted required CI on the final pushed SHA remains pending
for the controller; this task does not poll CI or accept delivery.
