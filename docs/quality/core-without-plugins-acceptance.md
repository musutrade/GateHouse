# Core build and tests without plugin sources (isolation acceptance)

Status: **pass** (2026-10-12), Linux amd64. With the plugin implementation paths
removed, core builds, every cargo test passes and the corrected core Python
selection passes without plugin source.

- Commit: `7c922cc234c436b1a0604be222f24d0bfa1f501d`.
- Entry point: `python3 tools/quality/core_isolation_check.py --commit <full sha>`
  (manual acceptance tooling; not wired into any workflow and not part of the
  required aggregate).
- Method: `git archive` snapshot into a temporary directory; the 24 plugin
  implementation paths are removed and their absence asserted; a local git HEAD
  is created inside the copy because quality tools record commit metadata;
  required core inputs are checked explicitly and fail the run when missing;
  cargo runs with an isolated `CARGO_TARGET_DIR` and the existing dependency
  cache; the source repository is never modified.

## Measured results

| Step | Result |
| --- | --- |
| `cargo build --locked --manifest-path tools/harness-gate/Cargo.toml` | pass |
| `cargo test --locked --manifest-path tools/harness-gate/Cargo.toml` | pass — 492 tests, 0 failed, 0 skipped |
| Python core selection | pass — 43 modules, 426 tests, 426 passed, 0 failed, 0 skipped |
| Plugin-owned test modules | 15 excluded, each with an ownership reason in the entry point |
| Plugin implementation paths removed | 24 (directories and root modules) |

The pinned measurer is the only analyzer source: `ci_quality.ensure_measurer`
downloads the release asset, verifies its recorded digest, bounds extraction to
the single release root, requires the executable bit and the declared
executable, and `test_source_measure` plus the pin test run it on Linux amd64.
No test falls back to building plugin source; the paths do not exist in the
copy.

## Corrections applied for this result

| # | Coupling | Fix |
| --- | --- | --- |
| 1 | `rust_reference.py` and `rust_equivalence.py` were excluded although they are core reference-equivalence tooling | restored to the core selection with their tests and fixtures; 19 tests pass in isolation |
| 2 | 20 local links in core documents pointed at plugin implementation paths | migrated targets link to the GateProbes snapshot `155b233fb2f525f520b4fcc0ed73e017154345e0`; historical evidence links to the Harness-Gate historical commit `8d93ea53c7cf8b314bda816eafe13209da25c742` instead of new-repo main |
| 3 | the pin test read the plugin source `plugin.json` | verifies the pinned release digest, extraction and executable boundaries, the released metadata and the analyzer/version/rule identity actually returned by the binary |
| 4 | `test_source_measure` built the plugin analyzer and `source_measure.provenance` hashed plugin `Cargo.toml`/lock/sources | both use the verified pinned release; provenance records tag, asset, digest and analyzer identity; `SERIES` is unchanged and no source hash is fabricated |
| 5 | the gh-146 inventory required every module, so a core-only tree failed | each plugin-owned absence needs the per-file `plugin_owner` approval; core, frozen and oracle rows must exist; unregistered files fail; negative tests cover missing core/oracle/frozen rows, unapproved absences, unregistered files and duplicate rows |
| 6 | the supplemental `rust-measure-native` workflow built the plugin analyzer from core source on Windows/macOS | retired to `retired/workflows/` with restore commit `4ec65a7666511cb80c89d7d369a56ec71cb878df`; cross-platform analyzer builds are plugin-owned |

## Verified scope and non-claims

- Verified: the exact commit above builds and passes every cargo test and the
  core Python selection with the plugin implementation paths absent, using the
  locked dependencies and an isolated build directory on Linux amd64.
- Not verified: other host platforms, any release chain, signing identity,
  performance baseline or the paused bundled product. Windows/macOS analyzer
  validation was retired with `rust-measure-native` and is now plugin-owned, so
  this record does not claim cross-platform core acceptance.
- Required CI still checks out the full repository; this entry point is the
  repeatable acceptance for the isolation gap, not an always-on gate.
