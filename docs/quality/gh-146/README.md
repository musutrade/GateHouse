# GH-146: Python boundary and compatibility freeze

Scope: tasks 1.1–1.2 of [the OpenSpec change](../../../openspec/changes/consolidate-generic-quality-core-into-rust/tasks.md).

The [31-module inventory](python-boundary.md) records purpose, callers, CI role,
release impact, current authority and final disposition. The
[shared corpus](../../../tools/quality/fixtures/generic-core/README.md) freezes
33 retained native-derived positive/negative inputs and full expected generic
results, with schemas, canonicalization and source/raw artifact bytes.

[ADR-0040](../../adr/0040-language-agnostic-evidence-policy.md) retains the existing
Rust required path as the sole release authority. No Rust product source,
workflow, required-check name/dependency, branch protection or runtime evaluator
changes in this issue. Generic Rust implementation, differential acceptance and
authority transfer remain unchecked tasks.

Local evidence is recorded in [validation.json](https://github.com/musutrade/Harness-Gate/blob/d9a932b23dcfea709175285b594ca200450a9971/docs/quality/gh-146/validation.json) and
[validation.tar.gz](https://github.com/musutrade/Harness-Gate/blob/d9a932b23dcfea709175285b594ca200450a9971/docs/quality/gh-146/validation.tar.gz). It includes pre-change Python behavior,
final required local checks, oracle replay, strict OpenSpec validation and an
unchanged-authority comparison. Cargo uses workspace-local `target/build`.
This checkout has no `.harness-gate/flow.toml` declaring a `ci` profile; config
check and CI-profile verify are not applicable, not passed. Hosted required CI
must pass on the final pushed SHA before the controller merges this PR.
