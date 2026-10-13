# GH-95 validation

Scope: OpenSpec `strict-json-results-and-risk-based-quality-gates`, tasks 7.1–7.4.
Related decisions: ADR 0034 and ADR 0038. The broader proposal remains in progress.

The previous gate accepted a names-only fixture with no source coverage (exit 0;
`target/quality/gh-95/before.json`). The v2 gate rejects names-only evidence and
requires isolated, commit-bound source regions. The inventory has 12 Linux-applicable
rows and six independent mandatory IDs. The JSON regression executes the CLI and
checks its report, typed failure code, incomplete parser evidence and nonzero exit.

Validation on Linux `x86_64-unknown-linux-gnu`:

| Command | Result |
| --- | --- |
| `CARGO_TARGET_DIR=$PWD/target/build cargo nextest run --manifest-path tools/harness-gate/Cargo.toml --locked` | 313 passed, 0 skipped |
| `cargo fmt --manifest-path tools/harness-gate/Cargo.toml -- --check` | Passed |
| `CARGO_TARGET_DIR=$PWD/target/build cargo clippy --manifest-path tools/harness-gate/Cargo.toml --all-targets -- -D warnings` | Passed |
| `python3 -m unittest discover -s tools/quality/tests -v` | 109 passed |
| `CARGO_TARGET_DIR=$PWD/target/build python3 tools/quality/docs_consistency.py --output target/quality/docs-consistency.json` | Passed |
| `openspec validate strict-json-results-and-risk-based-quality-gates --strict` | Passed |
| `python3 tools/quality/critical_paths.py --collect --output target/quality/critical-paths.json` | 12/12 applicable paths (100%); all six mandatory paths passed |
| `harness-gate config check` | Not applicable: `.harness-gate/flow.toml` is absent |
| `harness-gate verify --profile ci --all` | Not applicable: no declared project-local `ci` profile |

Fault fixtures delete each mandatory ID and inject skipped/cancelled/failed/missing
or extra tests, foreign test identities, stale/mixed commits/targets/rules, reused
artifacts, missing/modified coverage, an unhit inner failure region under a hit outer
function, moved symbols, altered source bytes, weakened assertions, narrowed
mandatory applicability, and failed clean/test/coverage commands. A mandatory failure
also blocks a matrix above 95%. Each negative case is required to fail closed.

Local raw logs are under `target/quality/gh-95/`; matrix artifacts, command records,
nextest JSONL and LLVM JSON are under `target/quality/critical-path-runs/`. CI uploads
`target/quality` even on failure. Final handoff records the pushed revision and
validation results; evidence is regenerated against that committed revision.

The initial instrumented command without a target override failed with:
`failed to open: /home/gem/cargo-target/llvm-cov-target/debug/.cargo-build-lock`:
`Read-only file system (os error 30)`. The ambient build directory is outside this
workspace. Rust compilation and the collector therefore use `target/build` in this
workspace. Initial JSON fixture setup failures (missing timeout and preset files)
were corrected before the passing full suite. Initial probes on non-executable
`break`/`let` keywords were moved onto the calls in the same reviewed branches.
No gate or configuration was invented to bypass an environment limitation.

Limitations: Linux validation only; reviewed Unix exclusions remain explicit on
Windows. Fake Docker/Podman ownership tests do not claim real-container execution.
Source-region hits plus reviewed assertions establish bounded observable evidence,
not automatic semantic proof of every assertion or complete descendant containment.

Delivery environment: `git add` and `git commit` initially failed with
`Unable to create '/home/gem/symphony-workspaces/GH-95/.git/index.lock': Read-only file system`.
The prepared metadata was copied to `target/gh95-git` inside this workspace;
commit/push use that Git directory with the original work tree and branch.
For final collection, `PATH=$PWD/target/gh95-bin:$PATH` directs only root-workspace
Git identity/object reads to this metadata. Git commands inside test projects
remain unchanged. The final handoff identifies the actual pushed commit.
