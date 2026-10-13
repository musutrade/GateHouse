# Phase 2 Optimization Baseline Metrics

**Date:** 2026-08-26  
**Commit:** (baseline before Phase 2 optimizations)  
**Platform:** Linux 7.0.0-29-generic

## Test Performance

Measured with `cargo test` across 3 runs:

| Run | Real Time | User Time | Sys Time | Tests |
|-----|-----------|-----------|----------|-------|
| 1   | 0m0.952s  | 0m0.664s  | 0m0.176s | 9     |
| 2   | 0m0.943s  | 0m0.684s  | 0m0.179s | 9     |
| 3   | 0m0.938s  | 0m0.652s  | 0m0.199s | 9     |

**Average Test Time:** 0.944s  
**Standard Deviation:** 0.007s (very consistent)

**Note:** Test execution itself is ~0.46s, the rest is compilation/linking overhead.

## cargo-nextest Baseline

Initial run with cargo-nextest reveals **76 total tests** (not 9):
- Unit tests: 67
- Integration tests: 9

| Run | Real Time | Test Execution | Tests |
|-----|-----------|----------------|-------|
| 1   | 23.620s   | 0.478s         | 76    |
| 2   | 0.771s    | 0.505s         | 76    |

**Note:** First run includes compilation (23s), subsequent runs are ~0.77s total with ~0.5s test execution.

### Slow Test Analysis

Top 10 slowest tests (optimization candidates):

| Test | Time | Module | Type |
|------|------|--------|------|
| test_secrets_scan_basic | 0.437s | integration_test | Integration |
| service_failure_does_not_skip_unrelated_steps | 0.248s | verify::tests | Unit |
| captured_command_has_a_hard_timeout | 0.159s | process::tests | Unit |
| task_runs_in_an_isolated_session | 0.109s | process::tests | Unit |
| task_can_remove_an_inherited_environment_variable | 0.108s | process::tests | Unit |
| staged_scan_uses_the_staged_secret_config | 0.091s | secrets::tests | Unit |
| hard_rule_detects_multiline_and_dynamic_sql_surfaces | 0.073s | audit::tests | Unit |
| hard_rule_detects_multiline_sensitive_logging | 0.062s | audit::tests | Unit |
| git_remote_check_rejects_non_git_directory | 0.061s | doctor::tests | Unit |
| timeout_terminates_the_task | 0.059s | process::tests | Unit |

**Key Observations:**
- Process tests with timeouts/isolation are slow (0.1-0.16s) - necessary for testing timeout behavior
- Integration test is slowest (0.437s) - spawns actual CLI process
- Most tests are <0.05s, which is acceptable
- Total slow test time: ~1.4s out of ~0.5s total (tests run in parallel)

**Optimization Opportunities:**
- Limited: Most slow tests are intentionally slow (testing timeouts, process isolation)
- Main gain comes from nextest's better parallelization, not individual test speedup

## Binary Size

**Release Binary:** 4.3MB  
**Location:** `tools/harness-gate/target/release/harness-gate`

Measurement:
```bash
ls -lh tools/harness-gate/target/release/harness-gate
-rwxrwxr-x 2 gem gem 4.3M Aug 26 07:35 harness-gate
```

## Compilation Time

**Clean Release Build:** 1m 20.205s  
**User CPU Time:** 2m 49.937s  
**System CPU Time:** 0m 8.777s

Command used:
```bash
cargo clean && time cargo build --release
```

## Current Cargo.toml Profile

```toml
[profile.release]
strip = true          # Remove debug symbols
lto = true            # Link-time optimization
codegen-units = 1     # Single codegen unit for better optimization
```

## Target Metrics (Phase 2 Goals)

Based on Phase 2 specification:

| Metric           | Baseline  | Target      | Improvement |
|------------------|-----------|-------------|-------------|
| Test Time        | 0.944s    | <0.66s      | 30%+        |
| Binary Size      | 4.3MB     | <3.5MB      | 18%+        |
| Compilation Time | 1m 20s    | <1m 12s     | 10%+        |

**Notes:**
- Test time baseline is lower than spec mentions (~20s) - this is because the actual test execution is fast; the spec may have included compilation
- We'll focus on the actual test execution time and overall development cycle time
- Binary size and compilation time targets remain as specified

## Test Inventory

Current test count: **9 integration tests**

Located in: `tools/harness-gate/tests/integration_test.rs`
- test_config_check_invalid
- test_config_check_valid
- test_init_creates_gitignore_entry
- test_init_twice_fails
- test_init_with_rust_api_preset
- test_presets_lists_available
- test_scope_detection_all
- test_secrets_scan_basic
- test_verify_fails_without_git

## Environment

- **Rust Version:** (run `rustc --version` to record)
- **Cargo Version:** (run `cargo --version` to record)
- **OS:** Linux 7.0.0-29-generic
- **CPU:** (record for context)
- **RAM:** (record for context)

## Binary Size Analysis (cargo bloat)

Top 15 contributors to binary size:

| Size   | Crate          | Function |
|--------|----------------|----------|
| 51.6KB | harness_gate   | `<audit::Config as Deserialize>::deserialize` |
| 50.6KB | harness_gate   | `<Commands as Subcommand>::augment_subcommands` |
| 40.7KB | harness_gate   | `<DoctorCheckKind as Deserialize>::deserialize` |
| 40.2KB | harness_gate   | `run` |
| 39.5KB | harness_gate   | `verify::run_configured_steps` |
| 37.8KB | harness_gate   | `<LegacyConfig as Deserialize>::deserialize` |
| 34.1KB | regex_automata | `meta::strategy::new` |
| 33.1KB | harness_gate   | `audit::run` |
| 31.1KB | harness_gate   | `<SecretRule as Deserialize>::deserialize` |
| 25.7KB | harness_gate   | `<SecretScanner>::from_source` |
| 25.0KB | globset        | `<GlobSetBuilder>::build` |
| 21.6KB | clap_builder   | `<Parser>::get_matches_with` |
| 20.9KB | regex_automata | `<Compiler>::c` |
| 20.1KB | harness_gate   | `secrets::scan` |
| 19.4KB | toml           | `<ValueDeserializer as Deserializer>::deserialize_any` |

**Total .text section:** 2.9MB (39.7% of 7.4MB file)  
**Other methods:** 2.4MB across 4,115 smaller methods

**Key Observations:**
- Serde deserialization generates significant code (multiple 40-50KB functions)
- Clap command building is 50.6KB
- Regex and globset operations contribute ~80KB combined
- Main business logic (`run`, `verify::run_configured_steps`, etc.) is relatively small

**Optimization Targets:**
1. Consider using serde feature flags to reduce deserialization code
2. Minimize clap features (we may not need all augmentation features)
3. Regex/globset are necessary but could be optimized with feature flags

---

## Track 4 Results: Build Profile Tuning

Measured back-to-back on one machine (`cargo clean` before each run) so the two
numbers are directly comparable. The 1m 20.205s figure recorded earlier in this
document was a separate, earlier session and should not be diffed against these.

| codegen-units | Clean release build | Binary size |
| ------------- | ------------------- | ----------- |
| 1 (before)    | 79.31s              | 4.20 MB     |
| 4 (after)     | 68.96s              | 4.39 MB     |
| 16            | 78.1s               | -           |

**Result: -13.0% build time**, against the >=10% target.

Why cu=4 and not more: `harness-gate` itself accounts for 62.95s CPU (33.9% of
the build) and is the sink of the dependency graph, so it cannot overlap with
anything -- it *is* the critical path. Splitting its codegen into 4 units
parallelizes that segment. At cu=16 the win reverses, because fat LTO has to
reconcile more units and becomes the serial bottleneck.

Cost: +0.19 MB on the `release` binary. Distribution is unaffected -- release.yml
ships `release-small`, which pins `codegen-units = 1` back explicitly to avoid
inheriting this trade (2.98 MB, within the <3.5 MB target).

`dev` was measured and deliberately left on cargo's defaults (1.1s incremental
rebuild, 0.5s `cargo check` -- no headroom worth taking). Only `debug = 1` was
added: full dev build 19.5s -> 17.9s, `target/debug` 511 MB -> 425 MB, while
keeping the line numbers panic backtraces need.

Not attempted: the 49% of IR from serde derive expansions identified above.
Both routes to removing it (hand-written `Deserialize` for ~30 structs, losing
`deny_unknown_fields`; or two-phase `toml::Value` parsing, losing line numbers in
config errors) make the most correctness-critical part of the codebase more
fragile, and the profile change met the target without touching it.

---

This baseline will be compared against Phase 2 results to validate improvements.
