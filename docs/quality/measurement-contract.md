# Measurement contract (candidate)

This record freezes how the strict-JSON quality change compares measurements.
It is evidence about the measurement procedure; it does not change the
Harness-Gate runtime.

## Identity

| Field | Frozen value |
| --- | --- |
| Review baseline | `c9101c3191be7a5fd639c64c3781ccb154e0ce34` (`v0.3.7`) |
| Candidate measured commit | `ce9e0cefb2b373e8c8bde018b53b63d0d8c9d8c6` |
| Target triple | `x86_64-unknown-linux-gnu` |
| Test and coverage profile | Cargo `dev` (debug) |
| Distribution benchmark profile | Cargo `release-small` |
| Cargo target directory | `/home/gem/cargo-target` in this workspace; use `cargo metadata` rather than assuming a path |
| Rust | `rustc 1.97.1 (8bab26f4f 2026-07-14)` |
| Cargo | `cargo 1.97.1 (c980f4866 2026-06-30)` |
| cargo-nextest | `0.9.143 (60fa45f63 2026-08-04)` |
| cargo-llvm-cov | `0.9.0` |
| OpenSpec CLI | `1.10.0` |

The review commit is recorded in the proposal but is not present in this
shallow checkout. It must be fetched or supplied by the controller before a
baseline can be accepted; the candidate commit above is not a substitute.

## Commands and binary provenance

Run from the repository root in a clean checkout:

```bash
python3 tools/quality/measurement_contract.py \
  --output target/quality/measurement-contract.json
cargo nextest run --manifest-path tools/harness-gate/Cargo.toml --locked
python3 tools/quality/coverage.py \
  --output target/quality/coverage.json
```

`measurement_contract.py` runs `cargo nextest list --locked --message-format
json`, records every test-binary path, byte count, and SHA-256, and records the
Cargo target directory returned by `cargo metadata`. This prevents a test
result from being attributed to a stale or different binary. The nextest run
and coverage run are separate measurements and must not be merged by test-name
or wall-clock time.

Coverage is produced by `cargo-llvm-cov` with `-C instrument-coverage` and a
profile pattern such as `harness-gate-%p-%8m.profraw`. Child processes inherit
`LLVM_PROFILE_FILE`, but they are counted only if the child binary itself was
built with the same instrumentation and its `.profraw` is retained. Missing
child profiles are a measurement error, not zero or full coverage.

Branch coverage is `unsupported` for this baseline because the command does
not pass `cargo llvm-cov --branch`; line and region evidence remain distinct.

## Coverage interpretation

- File-level coverage divides executable lines in one file.
- Module-level coverage sums covered and executable lines for one declared
  source prefix.
- The blocking aggregate sums the six blocking module counters.
- Service coverage is reported separately as informational.
- Raw LLVM totals include files outside those boundaries.

Therefore the historical `23.38%` (`doctor/checks.rs`), `24.83%`
(`project/input.rs`), `78.33%` raw LLVM total, `83.27%` six-module aggregate,
and `70.96%` service observation are different measures and cannot be
substituted for one another.

For the candidate commit, the documented `coverage.py` command produced
`9,286/11,090 = 83.7330928765%` for the six-module aggregate; service was
`1,373/1,935 = 70.9560723514%` and remained informational. The unrounded
counters are retained in the generated JSON and are the values used for the
threshold decision.

The machine-readable candidate is generated at
`target/quality/measurement-contract.json` (ignored by Git) and should be
reviewed with the retained raw profiles and coverage artifacts.

## Local validation note

The repository itself is a source checkout, not an initialized Harness-Gate
fixture. Consequently these target-project commands cannot run here:

```text
$ harness-gate config check
ERROR: could not find project root above /home/gem/symphony-workspaces/GH-89; expected .harness-gate/flow.toml; run `arc-flow init --preset <name>`

$ harness-gate verify --profile ci --all
ERROR: could not find project root above /home/gem/symphony-workspaces/GH-89; expected .harness-gate/flow.toml; run `arc-flow init --preset <name>`
```

This only limits target-project flow validation; it does not affect the Rust
format, Clippy, nextest, OpenSpec, or measurement-contract checks.
