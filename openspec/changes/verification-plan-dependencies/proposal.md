# Proposal: Verification Plan Dependency Foundation

## Goals

- Add a backwards-compatible dependency field to configured steps.
- Validate and deterministically order the selected dependency graph.
- Establish the internal boundary needed by a later scheduler.

## Non-goals

- No parallel execution, cancellation policy changes, service locking, or new CLI syntax.

## Success Metrics

- Legacy presets execute in their existing order.
- Missing, self, and cyclic dependencies fail during config validation.
- Selected steps include prerequisites in stable topological order.

Risk: Medium-low; execution remains serial and the field is optional.
