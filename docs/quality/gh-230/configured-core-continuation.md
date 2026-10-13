# GH-230 configured Core invocation continuation

> Storage update (2026-10-13): historical raw logs/bundles are available through
> the [verified archive guide](archive.md). Dates, digests and outcomes below are unchanged.

Status: partial diagnostic; P6/P7 remain incomplete. All relevant Engineering
Policy semantics remain unchanged.

The bounded authenticated configuration/request increment now exercises the
distributed Core's existing `quality compile` and `quality collect` commands.
The test prepares generic configuration and host-owned state from newly captured
native function identities, pins all configuration files, and signs the existing
adapter v2 request with a disposable test key. It adds no Rust-specific Core
dispatch or policy implementation. An empty policy table is appropriate to this
transport test; it makes no project policy or acceptance decision. Actual policy
decisions remain covered by the native/Core evaluation diagnostics recorded in
[the prior continuation](core-0.4.0-continuation.md).

The configured test verifies:

- Stale configuration identity and stale context fail before collector invocation.
- A changed nonce with the original signature fails verification before invocation.
- A valid signed request invokes the installed collector once. Its empty reviewed
  compatibility matrix rejects the unknown combination, and Core reports an
  operational collection failure without an output report or evidence artifacts.
- Replaying that request fails before another collector invocation.

An external test wrapper records configured collector invocations. The installed
entry is also invoked directly once to retain its exact rejection response;
that diagnostic call bypasses the counting wrapper. These counts establish only
the configured producer's invocation behavior. They do not measure native
descendant processes, prove absence of all possible fallback processes, or
establish clean-host producer counts. Original command records distinguish the
configuration calls from fixture captures and direct diagnostic calls.

The official Core path is
`target/symphony-inputs/core-v0.4.0/harness-gate-linux-amd64`, with SHA-256
`8e3df8303ca8f650d4ef768b29cfefb60ca115122a47b116245cc19e4649bbdd`.
The installed runtime is the preserved workspace-local assembly at
`target/gh-230/continued/runtime`. Each test execution makes fresh native fixture
captures in a new `target/gh-230/runtime-tests/` directory. No external workspace
captures, checkout-built Core or new download is used.

The first focused run failed because the new test configuration omitted the
required empty `policies` table. Its original log and generated fixture are
retained. Adding that table fixed configuration parsing; the second focused run
passed. Subsequent test coverage adds stale configuration and signature negatives.
The first full discovery ran 434 tests with one error and zero skips: the
private Cargo test lacked `RUST_COLLECTOR_TEST_VENDOR`, as in the previous
continuation's documented environment failure. The corrected run uses the
already-created `target/gh-230/continued/vendor`; no vendor download or source
change was needed. Both runs are retained.

The initial documentation check returned exit 1 for generated examples, migration
and schema synchronization. Its invocation omitted the established workspace
build and temporary-directory settings; the checker suppresses child diagnostics,
so that report alone does not identify a specific subprocess error. With the
previously validated local environment and proxy removal, the same command
returned exit 0. Both original reports are retained.


## Validation and retained evidence

| Command | Result |
| --- | --- |
| `python3 -m unittest test_rust_collector_runtime.StandaloneNativeTests.test_actual_core_configures_installed_collector_without_retry_or_fallback -v` | First focused run: one failure from omitted `policies`; corrected focused run: one passed. The test was subsequently renamed to `test_actual_core_configures_installed_collector_once_and_rejects_replay` and expanded. |
| `python3 -m unittest discover -s tools/quality/tests -v` | First discovery: 434 tests, one error, zero skips. Corrected discovery: 434 passed, zero skips; exit 0 (287.453 seconds wall time). |
| `python3 tools/quality/docs_consistency.py --output target/quality/docs-consistency.json` | Initial environment: exit 1; corrected workspace environment: exit 0. |
| `OPENSPEC_TELEMETRY=0 openspec validate package-official-rust-collector-for-independent-delivery --strict --no-interactive` | Exit 0. |
| `git --git-dir=target/gh-230/continued/git-metadata --work-tree=. diff --check` | Exit 0. |

Rust source is unchanged in this continuation. The preceding nextest (392 passed,
zero skips), fmt and clippy results remain in the
[official Core continuation](core-0.4.0-continuation.md). Repository-level
`harness-gate config check` and `harness-gate verify --profile ci --all` remain not
applicable: this repository has no `.harness-gate/flow.toml`. The disposable test
project has its own configuration and does not change that applicability.
Required hosted CI has not run for this unfinished work.


[Exact commands and environments](https://github.com/musutrade/Harness-Gate/blob/d9a932b23dcfea709175285b594ca200450a9971/docs/quality/gh-230/configured-core-checks.json) and
[original validation logs and tested source snapshots](archive.md)
retain the focused runs, both full discovery runs, and documentation checks.
The successful full run includes all nine standalone native tests. For a future
run, use the corrected discovery's complete environment, including the existing
`RUST_COLLECTOR_TEST_VENDOR` path; do not repeat the omitted-setting failure.

The [initial evidence archive](https://github.com/musutrade/Harness-Gate/blob/d9a932b23dcfea709175285b594ca200450a9971/docs/quality/gh-230/configured-core-initial-evidence.tar.gz.parts/manifest.json) retains
1,557 original files (69,907,309 compressed bytes), with a per-file
[inventory](https://github.com/musutrade/Harness-Gate/blob/d9a932b23dcfea709175285b594ca200450a9971/docs/quality/gh-230/configured-core-initial-inventory.json) and
[verified archive receipt](https://github.com/musutrade/Harness-Gate/blob/d9a932b23dcfea709175285b594ca200450a9971/docs/quality/gh-230/configured-core-initial-archive.json).
The [corrected discovery evidence](https://github.com/musutrade/Harness-Gate/blob/d9a932b23dcfea709175285b594ca200450a9971/docs/quality/gh-230/configured-core-corrected-evidence.tar.gz.parts/manifest.json)
retains 1,431 original files (71,702,622 compressed bytes), with its
[inventory](https://github.com/musutrade/Harness-Gate/blob/d9a932b23dcfea709175285b594ca200450a9971/docs/quality/gh-230/configured-core-corrected-inventory.json) and
[verified archive receipt](https://github.com/musutrade/Harness-Gate/blob/d9a932b23dcfea709175285b594ca200450a9971/docs/quality/gh-230/configured-core-corrected-archive.json).
Every archived file was read back and checked against its SHA-256. Disposable
signing keys in these test archives are fixture inputs and authorize no release.
These archives retain local native diagnostics, not complete delivery-package
or clean-host acceptance evidence. Previous archives and failures are unchanged.

Documentation reports still read the original read-only `.git` at predecessor
`e24aeb3`; the working files and preserved local metadata include `ee54469`, as
explained in the prior continuation. No pushed commit or controller acceptance
is represented by these uncommitted records.

The [container access recheck](container-access-recheck.md) remains the acceptance
blocker: the approved wrapper cannot access `/var/run/docker.sock` from the model
sandbox. No repeat Docker probe was made without an infrastructure change.
Observed delivery preflight, successful installed generic measurement, complete
clean-host re-export, relocation acceptance, and cold/warm/peak-disk/native
producer measurements remain outstanding. The reviewed supported matrix remains
empty. Fixture results do not certify Arc-Admin, and no #215 action, publication,
baseline acceptance, task completion or controller handoff is claimed.
