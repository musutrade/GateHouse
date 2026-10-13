# ADR 0010: Decompose the Verification Module by Responsibility

## Status

**Accepted** - 2026-08-27

## Context

`tools/harness-gate/src/verify.rs` combined verification errors and reports,
gate orchestration, configured-step execution, result parsing, console output,
and tests in one 485-line module. Execution and parsing evolve independently,
and the combined file made changes to either path harder to isolate.

## Decision

Split verification implementation into private submodules:

- `verify/mod`: stable errors, report model, gate orchestration, and public
  entry points.
- `verify/steps`: configured service setup, task construction, execution, and
  result output.
- `verify/parser`: configurable test-result parsing and ANSI normalization.
- `verify/tests`: verification unit tests.

Keep the existing `crate::verify` exports, report files, output text, error
codes, and step-selection behavior unchanged. Child modules remain private
implementation details.

## Consequences

- Gate orchestration is separated from task execution and parser details.
- Parser and service execution changes can be reviewed independently.
- Internal visibility must remain narrow when adding execution features.
- Cross-cutting workflow changes may touch the parent and steps modules.

## Alternatives Considered

- Leave the module monolithic: rejected because orchestration and execution
  have different ownership and change patterns.
- Split every verification step into a module: rejected because configured
  steps share one execution contract and would add unnecessary indirection.
- Expose a public verification library: rejected because Harness Gate is a
  binary crate and no external API is required.

## Related

- [ADR-0009](0009-secrets-module-decomposition.md)
- [OpenSpec: split-verify-module](../../openspec/changes/split-verify-module/proposal.md)
