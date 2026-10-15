# Submodule measurement coverage

Inventory baseline: `29aeb22b894a8a6add54154688500fefa3a4e977` (#60).
This inventory does not alter performance thresholds or established series.

| Existing measurement | Fixed workload / metric | Boundary |
| --- | --- | --- |
| Verification serial/parallel | `tools/quality/fixtures/benchmark`, two workers; wall seconds, observed concurrency and service reuse | CLI invocation and workflow work; fixture preparation and report archival outside timer |
| Scheduler overhead estimate | wall seconds minus sum of step durations, clamped at zero | Not an isolated scheduler CPU measurement; concurrent step durations overlap |
| Tests | cold and repeated warm nextest commands | Whole command, including its configured build/test costs |
| Release artifact | `release-small` binary size and identity | Build is preparation, not a configuration or rendering timing |
| Scope matcher | 600 paths, 48 rules, 100 iterations per sample | Repeated cached/uncached matching plus separate command wall time |

`tools/quality/benchmarks.py` records raw samples and count/min/max/median.
The `quality-baseline` workflow job runs only when classification requests full
CI and the event is `push`; it runs five samples on Linux, macOS and Windows,
with a 45-minute job timeout. A skipped PR job does not mean coverage is absent.

## Confirmed attribution gap

The current harness does not isolate configuration loading. `FlowConfig` loading
builds source diagnostics, parses TOML for migration checks, checks interpolation,
parses the typed configuration, applies environment overrides and validates it.
Whole verification timing cannot assign a regression to this work. The existing
scope timer starts after its configuration has been loaded.

The proposed supplement measures the actual production loader in a test-only,
explicitly invoked repeated loop, using a fixed configuration with no environment
interpolation. Two separate series are needed:

- In-memory parse/diagnostics/validation, with fixture generation outside timing.
- Loading the same bytes from an already-created file, including read cost.

The second is warm filesystem loading, not cold-disk latency. Neither series
should include Cargo compilation or process startup. Record those separately if
useful; never subtract noisy wall timings and label the difference CPU time.
Record input byte count and digest, repetitions, raw sample durations, commit,
Rust toolchain, platform and build profile. Environment-sensitive overrides must
be absent or explicitly captured. Use the existing statistics convention, retain
baseline series identity, and compare only identical fixture/profile/platform
conditions. No new required CI job or dependency is justified by this inventory.

Configuration loading is selected as the first supplement; no assertion is made
that it is the current dominant bottleneck. Scanner and report generation remain
attribution candidates, not measured hotspots. Renderer work must coordinate
with #50's split rather than introducing a conflicting production refactor.

## Reproduce the supplemental series

From the repository root:

```sh
python3 tools/quality/config_benchmark.py --output /tmp/gatehouse-config-benchmark.json
```

This explicitly runs an ignored test in the `release-small` test harness. It does
not time the shipped CLI and does not claim identical test/release panic behavior.
The existing Rust API fixture is 2,104 bytes with four steps at the inventory
baseline; the emitted input digest and byte count identify the actual input.
Each path is warmed, then measured in five batches of 100 loads with alternating
order. Allocation and drop are included; temporary file creation, equivalence
comparison, Cargo compilation and process startup are outside the batch timers.
Repository-root filesystem validation is deliberately disabled in both paths.

The supplemental runner removes the three documented global configuration path
overrides. Its build identity checks reject build-altering environment overrides
and Cargo build/compiler/target/profile/environment configuration; jobs and target
storage locations remain permissible. Cargo configuration file digests are recorded
as input provenance, not as proof of resolved compiler flags. Do not compare across
compiler, target, profile, fixture, configuration or machine changes. Compare raw
batches as well as medians and repeat on the same idle machine before diagnosing a
regression. This new series is informational; it neither replaces the existing
15% benchmark policy nor automatically establishes a new required threshold.

Results are local outputs, not repository inputs. Keep only the samples needed
for an active comparison and delete derived output after acceptance.
