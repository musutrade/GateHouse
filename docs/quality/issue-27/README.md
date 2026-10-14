# Issue #27: declarative runner inputs

Core no longer chooses executable or argument behavior by ecosystem name.
GateProbes owns `runners/cargo-test.toml`, the v1 migration tool and real Cargo
acceptance. See [ADR 0054](../../adr/0054-declarative-runner-inputs.md) and the
[GateProbes runner guide](https://github.com/musutrade/GateProbes/blob/main/docs/runners.md).
The paired local branches are `fix/issue-27-declarative-runners` in GateHouse and
`fix/gatehouse-27-cargo-runner` in GateProbes. The external changes must accompany
adoption of the new nested runner v2 contract; no released fixed pair is claimed.

## Checks

- 404 Core binary unit tests passed; the strengthened declaration cases also
  passed their focused rerun.
- 27 Core quality library tests, 21 CLI integration tests and nine failure-path
  integration tests passed.
- The service/isolation/shard integration test now compares declared thread
  arguments, child environment and report fields across all six combinations.
- 26 quality orchestration tests passed after expanding source inventory.
- Four GateProbes migration tests and 81 CI self-tests passed; documentation
  checks, Rust formatting, diff whitespace and generated schema parity passed.
- GateProbes real Cargo acceptance compares executed argv and Core reports,
  including rejection of conflicting user thread flags, and runs a changed
  external definition with an unknown ID on the same binary.

## Measurement boundary

`config/validation/steps.rs` was outside the required source inventory. It is
now included explicitly for both base and head, with the same pinned analyzer,
compiler configuration, instrumentation and LLVM joining rules. Existing
unchanged function debt is handled by the existing same-series ratchet; no
waiver, threshold or retained baseline is modified.

[Native certification](native-certification.json) binds the candidate production
source, digest-verified measurer and raw local artifacts. Sixty-four focused
config/argument tests passed in an isolated instrumented snapshot. The changed
runner validators have 100% line coverage and 100% / 96.5% region coverage. The
new declaration validator has 93.1% line and 90.5% region coverage. All three pass
CRAP <= 30 and the existing high-complexity coverage threshold. Three-target AST
checks certify parsing only; native LLVM collection was Linux amd64.

This focused certification supplements the implementation checks. It does not
replace complete hosted base/head collection and comparison, and is not a claim
that CI or release publication has passed.

## Migration and rollback

Core rejects runner v1 before spawning processes. The external tool writes a
separate runner v2 flow, preserves parsed policy/parser/isolation/shard/retry
values and original user arguments, and refuses overwriting the source. Review
and validate it with the new Core before adoption. Restore the original flow
and original Core together to roll back. Existing reference presets declare no
runners and retain their original commands.
