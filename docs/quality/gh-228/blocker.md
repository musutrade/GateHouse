# GH-228 runtime dependency blocker

> Storage update (2026-10-13): historical raw logs/bundles are available through
> the [verified archive guide](archive.md). Dates, digests and outcomes below are unchanged.

Historical status: blocked before standalone implementation. The operator recovery
on 2026-09-11 supplied the pinned archive and built the driver in this assigned
workspace. See [resumed runtime work](runtime.md) and [validation](validation.md)
for the superseding status. The remainder of this document is the original failed
attempt's record, including its then-incomplete tasks and environment observations;
it is preserved rather than rewritten as successful evidence. Relevant Engineering
Policy semantics remain unchanged.

## Accepted prerequisite

On 2026-09-11 the GitHub API confirmed GH-227 closed as completed, its PR #233
merged, and main at `b627bbc0e202b6e8a40354b731bd1a4291f31d7a`, matching the
assigned `symphony/GH-228` checkout. The controller's GH-227 delivery comment
records that merge and validated head `fa9bd0eed9a409ac8a60673cae5d650b418318bc`.
This is an environment dependency blocker, not an outstanding predecessor.

## Reproduced failure

From this workspace, with all output/build paths local:

```sh
CARGO_TARGET_DIR="$PWD/target/gh-228/cargo" python3 tools/quality/rust-native-driver/bootstrap.py --sysroot "$(rustc --print sysroot)" --output target/gh-228/native-driver
```

Stdout/stderr were redirected to `target/gh-228/logs/bootstrap.stdout` and
`bootstrap.stderr`. The tool reported:

```text
exec_command failed: ProcessFailed { message: "Network access to \"https://static.rust-lang.org:443\" was blocked by policy." }
```

The process stderr retains the complete traceback ending in
`urllib.error.URLError: <urlopen error Remote end closed connection without response>`.
The tool did not return a process exit code; none is asserted for this attempt.

The existing bootstrap requires the official archive
`https://static.rust-lang.org/dist/2026-07-16/rustc-dev-1.97.1-x86_64-unknown-linux-gnu.tar.xz`
with SHA-256 `0109304e1995cce9e3362208f5d4ec0944e52a2ddfbc0a85d4ce5bea5d3081ab`.
That source pin comes from the accepted bootstrap; its bytes could not be fetched
or independently verified in this execution. No alternate source or host tool was
substituted, and the network restriction was not bypassed.

The installed toolchain reports rustc 1.97.1,
commit `8bab26f4f68e0e26f0bb7960be334d5b520ea452`, LLVM 22.1.6.
Its component inventory includes LLVM tools but no `rustc-dev`, and its target
library directory has no `librustc_middle` artifact. A direct build confirmed
that the available installation cannot compile the native driver:

```sh
CARGO_TARGET_DIR="$PWD/target/gh-228/driver-build" RUSTC_BOOTSTRAP=1 cargo build --locked --manifest-path tools/quality/rust-native-driver/Cargo.toml
```

Exit 101; seven `E0463` errors for missing compiler-private crates. Full output is
retained under `target/gh-228/logs/driver-build.{stdout,stderr}`. The base toolchain
was read only; no global Rust component or default was changed.

## Inventory findings and limits

The static AST import inventory in `target/gh-228/import-inventory.json` includes
conditional imports and SHA-256 for each source. Its roots are the production
native driver, classifier, delivery contract, and existing native-to-Core bridge.
The bridge is inventoried to understand existing behavior, not selected as a
product policy engine. These roots reach eight local Python modules and standard
library modules, with no unresolved direct Python imports. This does **not** prove
dynamic imports, extension/shared-library closure, or redistribution compliance.
The schema files loaded through `quality_evidence.SCHEMA_DIR` are additional
runtime data dependencies; a Python import list alone is not a payload inventory.

Four reached helpers are C-class frozen modules: `collector_runner.py` (transport
and reference preflight), `harness_evidence.py` (normalization/integrity helpers),
`project_model.py` (identity helpers), and `quality_evidence.py` (shared schema and
legacy measurement integrity). All four hashes match the accepted GH-152 inventory,
as recorded in `target/gh-228/frozen-helpers.json`. The generic policy, ratchet,
cross-component and report oracles are not in this import graph. Their C-class
restrictions and the D-class historical replay restrictions in
[Python retention](../python-retention.md) remain unchanged.

The existing adapter uses ambient `cargo` and invokes its source script as a Cargo
wrapper; the bootstrap overlays its host sysroot with symlinks. Neither is a
standalone product implementation. The current certify CLI still returns 1 for
a complete threshold-failing report, as well as failing on measurement errors;
no arbitrary legacy exit has been translated into operational success.

The observed host is Linux x86_64, kernel `7.0.0-31-generic`, glibc 2.43. Its Python
is `/usr/bin/python3`, version 3.14.4, with shared runtime prefix `/usr`.
These are observations in `target/gh-228/runtime-availability.json`, **not** a
verified supported minimum ABI or an approved private Python build input.
A private Python distribution and complete runtime/license closure remain unproven.

No collector bundle, native capture, binary/profile/LLVM export set, two-build
comparison, or nondeterminism result exists from this attempt. No historical
external workspace or capture was accessed. Source inventory, partial build
outputs and genuine failure logs remain in the assigned workspace. The reviewed
compatibility matrix remains empty. Package identity and distribution signatures
still cannot establish capture trust or measurement-series compatibility.

## Required resumption input

The controller must provide the exact pinned `rustc-dev` archive in this workspace,
or an environment where its authorized official download is permitted. The
existing `bootstrap.py --archive PATH` verifies the pinned SHA without changing
the global toolchain. Resume dependency/license and private Python closure checks
after that prerequisite is available; supplying the archive alone does not finish
P2/P3 or establish clean-host acceptance.

Repository checks and exact command environments are retained in
[validation.json](https://github.com/musutrade/Harness-Gate/blob/d9a932b23dcfea709175285b594ca200450a9971/docs/quality/gh-228/validation.json), [the validation runner](run-validation.py),
and [complete logs](archive.md). Successful repository checks cannot
establish native positive measurement. No task completion,
delivery PR, `.symphony-handoff.json`, release publication, GH-215 enablement,
baseline acceptance or dependent-issue readiness is asserted by this record.

## Repository validation

The runner uses workspace-local `CARGO_TARGET_DIR` and `TMPDIR`, disables OpenSpec
telemetry, and removes HTTP proxy variables for direct localhost webhook tests.
It does not change the global environment. The source code and tests are unchanged.

| Command (arguments and environment also retained in the runner/JSON) | Actual result |
| --- | --- |
| `cargo nextest run --manifest-path tools/harness-gate/Cargo.toml --locked` | Exit 100: 40 passed, 1 failed, 351 not run. `test_scope_without_git` found the parent checkout because its temporary fixture is workspace-local. Original failure retained. |
| `cargo fmt --manifest-path tools/harness-gate/Cargo.toml -- --check` | Exit 0. |
| `cargo clippy --manifest-path tools/harness-gate/Cargo.toml --all-targets -- -D warnings` | Exit 0. |
| `python3 -m unittest discover -s tools/quality/tests -v` | Exit 0: 401 tests, 2 native-driver-dependent classes skipped; 218.869 seconds including process overhead. No standalone native positive claimed. |
| `python3 tools/quality/docs_consistency.py --output target/quality/docs-consistency.json` | Exit 0. |
| `openspec validate package-official-rust-collector-for-independent-delivery --strict --no-interactive` | Exit 0. |
| `git diff --check` | Exit 0. |
| `harness-gate config check`; `harness-gate verify --profile ci --all` | Not applicable: no `.harness-gate/flow.toml` declaring `ci`; not reported as passed. |

The environment-only nextest repair bounds Git discovery at the temporary parent
directory so an uninitialized fixture cannot discover this checkout's `.git`.
It preserves both workspace-local scratch and the original test assertion.
Exact retry commands:

```sh
env -u HTTP_PROXY -u http_proxy -u HTTPS_PROXY -u https_proxy -u ALL_PROXY -u all_proxy TMPDIR="$PWD/target/gh-228/tmp" GIT_CEILING_DIRECTORIES="$PWD/target/gh-228/tmp" CARGO_TARGET_DIR="$PWD/target/gh-228/cargo" cargo nextest run --manifest-path tools/harness-gate/Cargo.toml --locked -E 'test(test_scope_without_git)'
env -u HTTP_PROXY -u http_proxy -u HTTPS_PROXY -u https_proxy -u ALL_PROXY -u all_proxy TMPDIR="$PWD/target/gh-228/tmp" GIT_CEILING_DIRECTORIES="$PWD/target/gh-228/tmp" CARGO_TARGET_DIR="$PWD/target/gh-228/cargo" cargo nextest run --manifest-path tools/harness-gate/Cargo.toml --locked --no-fail-fast
```

Targeted retry: exit 0, one selected test passed. Full retry: exit 0, all 392 tests
passed, zero skipped, 191.090 seconds reported by nextest. Logs are
`nextest-scope-retry.log` and `nextest-final.log` in the archive. Hosted CI has not
been requested for this blocked, unfinished implementation.
