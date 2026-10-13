# ADR 0013: Decompose the Doctor Module by Responsibility

## Status

**Accepted** - 2026-08-27

## Context

`tools/harness-gate/src/doctor.rs` combined the serialized doctor report and
terminal rendering with command, path, environment, Git, glob, version, and
service checks in one 291-line module. Report formatting and individual check
implementations change for different reasons and have different test seams.

## Decision

Split doctor implementation into private submodules:

- `doctor/mod`: stable `run` and `DoctorReport` exports.
- `doctor/report`: report data model, counters, and terminal rendering.
- `doctor/checks`: configured check dispatch and check-specific helpers.
- `doctor/tests`: doctor-specific behavior tests.

Keep the existing `crate::doctor::{run, DoctorReport}` boundary, serialized
field names, output formatting, check ordering, severity counters, timeout
handling, and error messages unchanged. Child modules remain private
implementation details.

## Consequences

- Report presentation and check execution can be reviewed independently.
- The parent module remains the compatibility boundary for CLI orchestration.
- New check kinds should be implemented in `doctor/checks` without coupling
  report formatting to check behavior.
- The small test-only visibility bridge for Git remote checks must remain
  internal to the crate.

## Alternatives Considered

- Leave the module monolithic: rejected because reporting and check execution
  have separate ownership and change risk.
- Create one module per check kind: rejected because it would add indirection
  without meaningful boundaries for the current check set.
- Introduce a public doctor-check trait: rejected because the binary crate
  does not need an extensibility API.

## Related

- [ADR-0012](0012-process-module-decomposition.md)
- [OpenSpec: split-doctor-module](../../openspec/changes/split-doctor-module/proposal.md)
