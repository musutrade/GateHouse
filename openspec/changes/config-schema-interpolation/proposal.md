# Proposal: Configuration Schema and Environment Interpolation

## Goals

- Export a JSON Schema directly from the v2 Rust model with `harness-gate schema export`.
- Support predictable `${NAME}` and `${NAME:-default}` values in every config loading path.
- Preserve existing CLI, validation, and dedicated environment override behavior.

## Non-goals

- No DAG scheduler, parallel execution, container-runtime abstraction, or report-format change.
- No recursive interpolation or expression evaluation.

## Success Metrics

- `config schema` writes valid `schema/flow.schema.json`.
- Missing variables fail with the variable name; defaults work.
- Existing test suite and configuration compatibility remain green.

Risk: Low. The change is additive and fails closed for malformed input.
