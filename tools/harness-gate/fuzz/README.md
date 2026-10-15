# Bounded result-parser mutation test

The ordinary test module [`parser_fuzz.rs`](../src/verify/parser_fuzz.rs) calls
production `parse_result_count` for JSON discovery, explicit JSON count paths,
JUnit and TRX. Existing deterministic parser regressions remain in place.
This is a reproducible mutation smoke test, not coverage-guided fuzzing or a
proof that arbitrary inputs cannot crash the parser.

For a fixed reference campaign, use Rust 1.99.0 (b940084d7) on
`x86_64-unknown-linux-gnu` with the checked-in Cargo.lock. From the repository
root, select that installed toolchain explicitly; no nightly toolchain,
cargo-fuzz or new dependency is needed. Ordinary required CI retains its existing
toolchain selection and runs the same bounded test:

```sh
rustc +1.99.0 -vV
cargo +1.99.0 nextest run --manifest-path tools/harness-gate/Cargo.toml --locked \
  --bin harness-gate --profile default -E 'test(bounded_result_parser_mutations)'
```

The source fixes the corpus and two nonzero u64 seeds. Each seed generates 256
inputs with 1–16 character insertions, deletions or replacements, then calls all
four parser configurations: **2,048 mutation calls**. Another **132 calls** check
known counts from 0 through 32. The generator uses Unicode scalar values because
the production entry accepts `&str`; invalid UTF-8 byte streams are outside this
harness's input domain. Seeds, case index, target and escaped input appear on a
caught panic, so reproduction does not depend on an external corpus directory.

Inputs have a 4,096-byte ceiling with UTF-8-safe truncation. The current short
seeds do not exercise that entire size boundary. Processing is in memory, one
input at a time, without subprocesses, service startup, file loading or entity
resolution. The default nextest profile enforces its existing 60-second slow
interval and termination after three intervals on the **whole test process**;
this is not a per-input timeout or an OS memory limit. Running through a different
test runner does not inherit that nextest timeout. `catch_unwind` reports ordinary
panics, not hangs, aborts or allocation failure.

Keep the failing input and original local failure output until a finding is
understood. Minimize it, add a deterministic regression for the production
behavior, then rerun that regression and this bounded test. Do not change seeds,
reduce the budget, weaken assertions or add retries to conceal a finding. A pass
means no unhandled finding within this declared budget. Temporary campaign logs
are local artifacts, not repository contents.

TOML loading was evaluated but deferred: its real entry interpolates environment
variables and applies process-environment overrides. A future TOML harness needs
an explicitly isolated environment rather than bypassing those production steps.
