# Trusted host migration inventory

This inventory accompanies [the trusted host](trusted-host.md). It does not grant
local coverage acceptance. Every row still belongs to the existing Actions job;
no current trigger, required dependency, threshold or routing rule changes here.

Source: [CI workflow](../.github/workflows/ci.yml) at repository revision
`19fc936e2d5d1fa54768c20bf44d3187b7c841dc`, workflow SHA-256
`69a811afc4ca0db934813945110fc0e28cc2411ffaab4f64762fa3ae068c24a8`.
Regenerate the comparison against the final deployed revision before acceptance.
The workflow itself contains the exact environment, pinned actions, command
arguments, fixtures and artifact retention requirements; this table identifies
which complete unit must be reproduced, rather than replacing its specification.

## Required execution cells

PR and main pushes, plus daily/manual full runs, use the existing impact routing.
Native build and native CLI contract jobs run outside PR events. Native tests do
run on applicable PRs. A routing-based skip is not a migrated native PASS. Local
full mode must cover every applicable cell before publishing its own success.

| Current job / environment | Configuration and complete workload | Evidence to accept | Local owner / acceptance |
| --- | --- | --- | --- |
| `test` / Linux | Stable Rust; locked default test/dev; pinned `arc-admin-replay` and `regression-corpus`; nextest; both findings-feedback and report-artifact Node test suites; synthetic HTML provenance/transport; workflow acceptance; both generated schemas; real locked GateProbes Rust/Python onboarding at `53c01505bed2ff61dfad1d106abbec2dcd961d05` | All test exits, synthetic HTML provenance and transport contract, workflow-acceptance directory, unchanged schemas, onboarding report/logs | Unassigned; pending |
| `test-cross-platform` / macOS | Native stable Rust, locked default tests, same two fixture groups; `cargo nextest run ... --no-fail-fast` | Native test results with actual platform/tool identity | Unassigned; pending |
| `test-cross-platform` / Windows | Native stable Rust, locked default tests, same two fixture groups; `cargo nextest run ... --no-fail-fast` | Native test results with actual platform/tool identity | Unassigned; pending |
| `security-audit` / Linux | Stable Rust; approved cargo-audit; `cargo audit --deny warnings` in `tools/harness-gate` | Audit result and advisory database identity/freshness at execution time | Unassigned; pending |
| `fmt` / Linux | Stable rustfmt; `cargo fmt --manifest-path tools/harness-gate/Cargo.toml -- --check` | Successful format check | Unassigned; pending |
| `clippy` / Linux | Stable Clippy; locked all-targets/all-features dev/test; `-D warnings` | Successful complete Clippy output | Unassigned; pending |
| `build` / Linux | Locked default release profile, existing LTO/codegen policy | Release build result and informational binary digest; binary is not new quality evidence | Unassigned; pending |
| `build-cross-platform` / macOS | Native locked release; existing `LTO=false`, `CODEGEN_UNITS=16` overrides | Native compile result and actual profile identity | Unassigned; pending |
| `build-cross-platform` / Windows | Native locked release; existing `LTO=false`, `CODEGEN_UNITS=16` overrides | Native compile result and actual profile identity | Unassigned; pending |
| `quality-fast` / Linux | Approved nextest/llvm-cov/llvm-tools; isolated instrumented profile; full pinned data; `ci_quality.py collect --stages legacy,production` with actual base/head/run | Both stage publications, tool identity, existing quality artifacts excluding build/snapshots | Unassigned; pending |
| `quality-risk` / Linux | Same instrumented tools/data; `collect --stages risk`; existing `CI_PARALLEL_RISK=1` semantics | Risk publication, complete existing CRAP/critical risk inventory and tool identity | Unassigned; pending |
| `quality-matrix` / Linux | Same instrumented tools/data; `collect --stages matrix` | Matrix publication and tool identity | Unassigned; pending |
| `quality-coverage` / Linux | Exact fast/risk/matrix inputs from this invocation; `merge`, `verify`, `ci_artifact.py seal` | Complete Core-accepted coverage/critical-path results, immutable manifest and all retained declared artifacts | Unassigned; pending |
| `quality-contracts` / Linux | Stable Rust; full pinned data; default dev; `contracts.py --output ...` | Existing CLI snapshot contract result and snapshots | Unassigned; pending |
| `quality-contracts-cross-platform` / macOS | Native stable Rust/default dev; full data; `contracts.py --structured` | Native structured contract result and snapshots | Unassigned; pending |
| `quality-contracts-cross-platform` / Windows | Native stable Rust/default dev; full data; `contracts.py --structured` | Native structured contract result and snapshots | Unassigned; pending |
| `docs-consistency` / Linux | Full mode installs Rust and executes `docs_consistency.py`; current docs-only pure route remains defined in workflow | Documentation/examples/schema result; pure mode cannot stand in for full mode | Unassigned; pending |
| `release-contracts` / Linux | Full pinned data; `python3 -m unittest discover -s tools/release/tests -v`; `bash tools/release/tests/test_install.sh` | Both complete governance and install-contract results | Unassigned; pending |
| `quality-scripts` / Linux | Full pinned data and Rust coverage tools; physical source-cleanup check; `run_core.py`; compile quality scripts | Complete Core-owned test discovery, cleanup result, all script tests and compilation results | Unassigned; pending |

## Routing, publication and advisory cells

| Current job | Role | Migration obligation |
| --- | --- | --- |
| `classify` / Linux | Repository-size gate, reviewed migration baseline, path classification; PR equivalence receipt and exact push-reuse proof | Preserve repository-size enforcement and applicable coverage decisions. Bind base/head/tree/merged input in the trusted invocation. Existing Actions reuse receipts do not authorize a host PASS. |
| `quality-required` / Linux | Existing stable required aggregate evaluates all declared dependencies and event/routing state | Retain unchanged during transition. Replace only after every applicable execution cell and live trust acceptance passes, using the actual Checks App integration ID. Do not introduce an Actions verifier as the replacement authority. |
| `quality-generic-shadow` / Linux | Advisory projection/comparison of this run's sealed retained Rust evidence | Retain advisory status and evidence validation; do not promote it to authority or use it to replace required measurements. |

Other workflows retain their existing release, scheduled or advisory ownership.
Inventory any applicable consumer-specific workflows before deploying there.

## Gaps in the initial Linux executor

The initial offline serial sandbox does not supply macOS/Windows execution,
GitHub action transports, an advisory-database refresh service, reviewed network
acquisition for pinned plugins, or a readable immutable Git history inside the
worker. Several existing quality scripts need that history. Schema generation
and onboarding also need writable working copies while the trusted candidate
remains immutable. These require reviewed step/materialization adapters and exact
evidence contracts before assigning a local owner. Running a smaller Linux suite
does not close those rows.

Record actual executor identity/platform, exact command/profile/configuration,
policy/toolchain digest, expected step IDs, evidence location, negative cases,
acceptance revision and responsible owner for each migrated cell. Update the
protected host coverage inventory from that reviewed matrix. An empty matrix or
an unassigned applicable row keeps `coverage_acceptance` in transition mode.

The separate Rust/frontend cache acceptance must record real project/revision,
full commands, cold/warm/source/toolchain/dependency/configuration scenarios,
compilation/test/maintenance wall time, logical/allocated disk and remote reporting
latency. No such production measurements are claimed by this inventory.
