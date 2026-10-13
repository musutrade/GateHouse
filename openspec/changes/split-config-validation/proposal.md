# Proposal: Split Configuration Validation

## Why

Configuration validation mixes cross-domain policy with reusable path,
identifier, template, and command safety checks.

## What Changes

- Keep `FlowConfig::validate` and cross-domain orchestration in `validation/mod`.
- Move primitive safety checks into `validation/primitives`.
- Move step/template/service-injection checks into `validation/steps`.
- Preserve validation ordering, errors, and public behavior.

## Success Metrics

- Existing tests pass without behavior changes.
- Format, Clippy, rustdoc, diff checks, and strict OpenSpec validation pass.
