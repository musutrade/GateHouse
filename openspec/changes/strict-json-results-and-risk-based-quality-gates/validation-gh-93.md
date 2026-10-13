# GH-93 failure-path validation

Scope: tasks **5.1–5.6** only, validated locally on Linux on 2026-09-07.
Dependencies GH-91 and GH-92 were closed with `state_reason=completed` before
this handoff. Remote CI and acceptance remain pending; this is not acceptance
of the whole proposal or a new quality baseline.

## Boundary evidence

The new [CLI integration suite](../../../tools/harness-gate/tests/failure_paths_test.rs)
launches the actual compiled binary in temporary Git projects. Assertions cover
exit codes, stdout/stderr, filesystem changes, process termination, and reports.

| Task | Executed evidence |
| --- | --- |
| 5.1 | `doctor_kinds_execute_success_required_failure_and_optional_warning`: all nine doctor kinds execute real checks; each has success, required failure, optional warning, and strict optional-warning exits. |
| 5.2 | `cli_selection_profile_alias_and_discovery_boundaries`: verify/check/step routing, explicit component selection, hook profile, invalid profile/step/component, incompatible flags, and upward discovery. Instrumented child execution reaches `app::run` 102 times and `app::commands::run` 57 times. |
| 5.3 | `staged_content_controls_result_and_snapshots_are_cleaned_after_failure`: staged failure with clean worktree and the inverse use actual Git-index content; invalid staged TOML fails before report creation. Execution-root removal and an empty isolated temporary directory prove cleanup. |
| 5.4 | `heartbeat_failure_blocks_release_and_retains_the_marker` now obstructs a real heartbeat filesystem read. `runtime_removal_failure_retains_evidence_and_allows_a_proven_retry` injects Docker/Podman removal failure through the runtime boundary, verifies retained marker bytes, zero removals with uncertain inspection, and successful cleanup after ownership is proven again. |
| 5.5 | `verification_cleanup_cancel_and_report_precedence_retains_resource_evidence` starts a service and a real task, then tests cleanup failure (`E1403`), SIGTERM plus cleanup uncertainty (`E1402`), and report-publication failure plus cancellation (`E1404`). Cancellation kills the task, retains resource/lease evidence, and makes zero removal calls when fresh ownership inspection is cancelled. Reports preserve plan order, separate task cancellation from cleanup failure, and retain sealed invocation artifacts even if the legacy mirror fails. `report_publication_error_precedes_gate_adapter_error_and_preserves_invocation_evidence` obstructs an actual audit-config read and legacy report path: typed `VerifyError::Report` wins, failed audit evidence remains explicitly incomplete, and no complete manifest is claimed. Existing gate failure, typed audit error, timeout, and deterministic primary-error ordering tests also pass. |
| 5.6 | `configured_runner_service_isolation_and_shards_match_the_executed_process`: six combinations of three isolation modes and optional sharding compare the actual process arguments/environment with report metadata, check service injection and environment removal, and confirm isolation allocation cleanup. Runner/environment collisions, invalid argument insertion, zero workers, and out-of-range shards exit before the probe runs and preserve previous reports. |

Sealed-report assertions independently recompute every manifest artifact's byte
count and SHA-256. Container fault injection uses a controlled executable at the
Docker CLI boundary, with actual service acquisition, inspection labels,
immutable container ID binding, lease files, cleanup, and report publication.
It is not evidence of successful operation against a real container daemon.
The subprocess suite is Unix-only; no Windows or macOS run is claimed.

## Reproduced defect and correction

Before the correction, `verify --all` accepted shard index 4 with total 3,
spawned the probe, and exited 0. The diagnostic configuration loader omitted
constraints already enforced by `FlowConfig::validate_execution`. It now calls
that existing validator when no specific execution-field diagnostic has already
been emitted. Existing field diagnostics retain their order; valid execution
and scheduler/report ordering remain unchanged. This also closes the same
loader's omitted execution retry/reference checks. There is no schema change.

This aligns the boundary with
[ADR-0026](../../../docs/adr/0026-configuration-safety-diagnostics.md) and preserves
the verification ordering described by
[ADR-0027](../../../docs/adr/0027-unified-verification-plan.md).
[ADR-0025](../../../docs/adr/0025-phase-1-quality-baseline-gates.md) and its accepted
measurement baseline are unchanged.

## Commands and results

All commands ran from the repository root. Cargo commands and docs consistency
used `CARGO_TARGET_DIR=target/gh-93-cargo` to stay in the writable workspace.

| Command | Actual result |
| --- | --- |
| `cargo nextest run --manifest-path tools/harness-gate/Cargo.toml --locked` | PASS: 308 tests, 0 failed, 0 skipped. |
| `cargo fmt --manifest-path tools/harness-gate/Cargo.toml -- --check` | PASS. |
| `cargo clippy --manifest-path tools/harness-gate/Cargo.toml --all-targets -- -D warnings` | PASS. |
| `python3 -m unittest discover -s tools/quality/tests -v` | PASS: 85 tests. |
| `python3 tools/quality/docs_consistency.py --output target/quality/docs-consistency.json` | PASS with the writable Cargo target override. |
| `openspec validate strict-json-results-and-risk-based-quality-gates --strict` | PASS. |
| `harness-gate config check` at repository root | Not applicable: `.harness-gate/flow.toml` is absent. Generated-project config checks are exercised by tests. |
| `harness-gate verify --profile ci --all` at repository root | Not applicable: no project-local flow file or declared `ci` profile. No generic config was invented. |

The first baseline nextest command inherited `/home/gem/cargo-target` and failed
with `Read-only file system (os error 30)` opening
`/home/gem/cargo-target/debug/.cargo-build-lock`. The failing Cargo subprocess
command and error are retained in `target/quality/gh-93/baseline.log`.
The same inherited target caused
the first docs-consistency attempt to report failed generated examples/schema
synchronization; its internal Cargo subprocesses suppress stderr. Both commands
succeeded after selecting the writable target; there is no remaining blocker.
The reproduced invalid-shard failure is retained in
`target/quality/gh-93/shard-debug.log`.

## Instrumented CLI provenance

```sh
CARGO_TARGET_DIR=target/gh-93-coverage cargo llvm-cov \
  --manifest-path tools/harness-gate/Cargo.toml --locked \
  --test failure_paths_test --json \
  --output-path target/quality/gh-93/cli-coverage.json
```

PASS: all five integration tests. The run retained 103 `.profraw` files,
including instrumented CLI children. The committed
[provenance record](evidence/gh-93-validation.json) binds the measured Rust
sources, instrumented executable, raw-profile inventory, coverage export, and
validation logs by SHA-256. Full artifacts remain in
`target/quality/gh-93/` and `target/gh-93-coverage/llvm-cov-target/` in this
workspace. The final pushed commit is recorded in the PR and Symphony handoff.

This isolated run proves execution of the app boundary; it is not the full
coverage gate or a CRAP measurement. Line and region counts are recorded
separately; branch coverage is unsupported for this invocation. Tasks outside
5.1–5.6, cross-platform acceptance, and quality-baseline acceptance remain
unchecked by this issue.
