# ADR 0054: Declarative runner inputs

Status: Accepted for Issue #27 implementation; no released fixed pair claimed.

## Problem

Core inserted Cargo/libtest flags based on `kind == "cargo-test"`. Both ordinary
and diagnostic validators required `program = "cargo"` for that name and exempted
it from thread environment injection. Consumers were `runner_inputs`, two
validator paths, runner tests and configuration examples. The external reference
presets have no runner declarations, so their commands remain unchanged.

## Decision

Runner contract v2 treats `kind` as an opaque identity. Definitions supply
`required_program`, `threads_args` and `threads_args_after`. Existing argument
positions and environment injection remain available. Thread templates support
only `{threads}` and must carry the declared budget. A separator is literal:
insert after its first occurrence, appending it if absent. User duplicate flags
remain untouched, preserving the child process's rejection/precedence behavior.

GateProbes owns `runners/cargo-test.toml` and its migration/acceptance tools. There
is no new Core plugin loader: external definitions become ordinary flow data.
Changing that data does not require recompiling Core. Unknown IDs use the same
validation and execution paths.

The nested runner version advances from 1 to 2; the enclosing flow stays v2.
All runner v1 declarations fail before execution with an explicit migration
message. GateProbes migration writes a new file and preserves non-runner values
and all existing runner fields except its version. Cargo v1 becomes explicit
Cargo executable and libtest thread declarations. Generic v1 receives only the
version update. No compatibility branch selects behavior by ecosystem name.

## Compatibility and rollback

Keep the old Core binary and original flow. Generate a separate v2 flow with
GateProbes `tools/runners/migrate.py`, then validate and review it with the new
Core. Roll back both binary and flow together. Released Core binaries accepting
only runner v1 cannot consume these declarations. Comments and formatting are
normalized by migration, but parsed configuration values are preserved.

Thread bounds, shared isolation restrictions, service environment collision
checks, parsers, test counting, shards, retries, requiredness and final decision
authority remain in Core. No policy, baseline or waiver relaxation is introduced.

## Verification

Core tests cover declaration validation through both validator paths, arbitrary
IDs, template errors, separator positions and duplicate user arguments.
GateProbes acceptance uses a temporary dependency-free crate and real Cargo,
captures executed argv, and compares it with Core runner/report records. It also
changes the external definition and runner ID while reusing the same binary.
