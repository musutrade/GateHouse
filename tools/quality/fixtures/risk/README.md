# LLVM function mapping fixture

`input.rs` was compiled and run locally for GH-92 with
`rustc 1.97.1 (8bab26f4f 2026-07-14)` and the toolchain's
`LLVM 22.1.6-rust-1.97.1-stable` coverage tools:

```bash
rustc -C instrument-coverage -C link-dead-code input.rs -o bin
LLVM_PROFILE_FILE="$PWD/run.profraw" ./bin
llvm-profdata merge -sparse run.profraw -o merged.profdata
llvm-cov export --instr-profile merged.profdata ./bin > llvm.json
```

The retained JSON is the real export, with only the absolute source filename
replaced by `@SOURCE@` and JSON whitespace normalized. The source bytes and
counter values are unchanged. `test_risk_bundle.py` substitutes the fixture's
current source path and maps it using the current locked analyzer. It verifies
two instances each of the generic function and its closure, one covered main,
and an unhit function. This detects LLVM's disjoint signature/body regions and
prevents duplicated instantiations from inflating source-function counts.

The complete-bundle tests use separate synthetic binaries/profile headers,
Git-source bindings and coverage files to exercise negative evidence handling.
Those synthetic fixtures are not a production baseline or proof of test execution.
