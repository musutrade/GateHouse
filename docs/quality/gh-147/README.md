# GH-147: Rust evidence and project semantics

Scope: tasks 2.1–2.2 of [the OpenSpec change](../../../openspec/changes/consolidate-generic-quality-core-into-rust/tasks.md).
The predecessor GH-146 merged as PR #153 with required CI complete.

The candidate [Rust library](../../../tools/harness-gate/quality-core/mod.rs)
implements `harness-evidence/v1` validation, all seven typed value variants, six
capability states, measurement-series identity and compatibility, provenance,
source/artifact hashes and byte counts. Project validation covers canonical
subject identity, component/target/boundary ownership, path containment, source
spans, duplicate and unknown references, and structural relationship constraints.
No ecosystem-specific policy branch is introduced.

The unchanged [33-case frozen corpus](../../../tools/quality/fixtures/generic-core/README.md)
and additional mutations supply 399 differential cases: 309 evidence, 46 project,
15 series and 29 capability requirements. Rust compares accepted values and
rejection classes with Python, including stale context, tampered bytes, unknown
metrics, incompatible series, malformed values and ambiguous identity. Domain
reasons match exactly; schema failures allow first-error versus aggregated wording,
and missing-file errors allow runtime-specific OS formatting. Additional tests
cover duplicate JSON keys, floats, Unicode/control characters, exact integers
beyond machine ranges, schema-copy equality and filesystem/symlink containment.

The new library is a default workspace member, so the required Cargo test command
runs its eight tests alongside the 315 existing tests. It is not a CLI dependency
or an authoritative evaluator. Existing coverage and risk collection explicitly
select `--package harness-gate`, preserving their CLI measurement boundary and
baseline. Policy, ratchets, cross-component evidence evaluation, reporting, replay
entry points and authority transfer remain tasks 3–7 under
[ADR-0040](../../adr/0040-language-agnostic-evidence-policy.md).

Actual local results and full logs are retained in [validation.json](https://github.com/musutrade/Harness-Gate/blob/d9a932b23dcfea709175285b594ca200450a9971/docs/quality/gh-147/validation.json)
and [validation.tar.gz](https://github.com/musutrade/Harness-Gate/blob/d9a932b23dcfea709175285b594ca200450a9971/docs/quality/gh-147/validation.tar.gz). Cargo uses workspace-local
`target/cargo-home` and `target/build` because the initial dependency download
failed with `Read-only file system (os error 30)` in the global Cargo cache.
The workspace-local retry succeeded. Python is required only for the candidate's
differential tests. The corpus is bounded reference evidence, not certification
of additional ecosystems or acceptance of later migration tasks.

This checkout has no `.harness-gate/flow.toml` declaring a `ci` profile, so
`harness-gate config check` and `harness-gate verify --profile ci --all` are not
applicable, not passed. Hosted required CI must pass on the final pushed SHA
before controller acceptance; it has not been polled or claimed here.
