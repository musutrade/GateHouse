# GH-228 resumed validation

> Storage update (2026-10-13): historical raw logs/bundles are available through
> the [verified archive guide](archive.md). Dates, digests and outcomes below are unchanged.

The accepted predecessor and original failed attempt are recorded in
[blocker.md](blocker.md). Operator recovery supplied the pinned rustc-dev archive;
[runtime.md](runtime.md) describes the resulting private runtime, host boundary,
license inventory, assembly commands and remaining delivery work. This record
does not confer CI, release, baseline or measurement-series acceptance.

## Exact checks

Run from the assigned checkout using [run-resumed-validation.py](run-resumed-validation.py).
The [combined result](https://github.com/musutrade/Harness-Gate/blob/d9a932b23dcfea709175285b594ca200450a9971/docs/quality/gh-228/resumed-validation.json) retains command arrays, exit codes,
elapsed times and command-local environment. `CARGO_TARGET_DIR`, `TMPDIR`, `GIT_CEILING_DIRECTORIES`, native driver,
Core binary and private runtime/capture paths are workspace-local. The six proxy
variables are removed for localhost webhook tests. `NATIVE_DRIVER_SYSROOT` is the
read-only installed compiler sysroot; private runtime tests use the bundled copy.

| Command | Actual result |
| --- | --- |
| `cargo nextest run --manifest-path tools/harness-gate/Cargo.toml --locked` | Exit 0; 392 passed, 0 skipped. |
| `cargo fmt --manifest-path tools/harness-gate/Cargo.toml -- --check` | Exit 0. |
| `cargo clippy --manifest-path tools/harness-gate/Cargo.toml --all-targets -- -D warnings` | Exit 0. |
| `python3 -m unittest discover -s tools/quality/tests -v` | Exit 0; 425 tests passed, including all seven private-runtime tests. |
| `python3 tools/quality/docs_consistency.py --output target/quality/docs-consistency.json` | Exit 0. |
| `openspec validate package-official-rust-collector-for-independent-delivery --strict --no-interactive` | Exit 0. |
| `git diff --check` | Exit 0. |
| `harness-gate config check` | Not applicable: this repository has no `.harness-gate/flow.toml` declaring `ci`. |
| `harness-gate verify --profile ci --all` | Not applicable for the same reason; no configuration was fabricated. |

The operator's original native run passed eight tests and skipped its Core CLI
integration because `HARNESS_GATE_NATIVE_POLICY_BINARY` was unset. Resumed checks
set that variable to the workspace-built actual Core CLI. Hosted required CI and
controller acceptance remain pending on the submitted PR.

## Native observations

The seven new runtime tests include an input-lock negative and six private-runtime
tests. Actual fixture compilation, profile merge, LLVM export, certification,
classification and Cargo capture all run with the private toolchain. A low-coverage
measurement completes with line counts 26/35 and CRAP 56/1, while the legacy CLI
keeps exit 1 and its retained `passed: false`. The actual Core CLI independently
returns aggregate failure from the same numeric facts. The frozen development
policy helper only prepares test inputs outside the bundle.

The complete native fixture classifies successfully after private LLVM re-export.
The existing Cargo fixture captures successfully offline using its original locked
dependencies, but classification correctly rejects its orphan/unloaded sources.
Wrong anchors and malformed/unknown development requests return errors with no
usable facts. These negatives do not replace the native positives. No installed
Core request, clean-host isolation, relocation equivalence or policy fallback is
claimed; those delivery boundaries remain governed by the later scoped tasks.

## Genuine failures retained

1. The original archive download was blocked and direct compilation returned
   seven E0463 missing compiler-private crate errors. Operator recovery resolved
   these inputs; the original logs remain unchanged.
2. The initial private capture failed because `cc` was unavailable on the isolated
   PATH. Adding the private GNU linker closure exposed missing LTO helpers; rust-lld
   reported an unknown empty plugin option. Private collect2/LTO inputs resolved it.
3. The attempted syscall trace failed because ptrace was denied by the sandbox.
   No syscall-trace or OS isolation success is claimed.
4. An assembly started before its lock-writing process finished and returned
   `FileNotFoundError`. The subsequent completed-lock assembly succeeded.
5. The first standalone Cargo test attempted network resolution with an empty
   project dependency cache and failed DNS. Staging the original locked project
   crate bytes and using an offline vendor input resolved this without adding
   ambient tools or altering project tests.
6. The next Cargo attempt built successfully but metadata returned exit 101:
   `rustc -vV` could not execute on the private PATH. Passing private `RUSTC` to
   metadata as well as compilation fixed the runtime, producing the final v6 lock.
7. The first full resumed Python run failed two tests (424 total): the new modules
   needed entries in the Python retention inventories, and the new Cargo assertion
   incorrectly expected complete classification from an intentionally incomplete
   existing fixture. The assertion now requires rejection and a separate complete
   native fixture establishes the positive. Business fixture bytes are unchanged.
8. The next full Python run executed 425 tests and failed its documentation test
   because this validation document was not yet present. All seven new runtime
   tests passed. The missing document was completed before the final rerun.

Full command logs and original native captures are retained in the evidence
[archives and inventory described in runtime.md](runtime.md#retention-and-validation). They retain errors and intermediate candidates;
no failure was rewritten as a pass. The runtime tar comparison covers repeated
assembly of pinned prebuilt inputs only, not compiler rebuild reproducibility.

## Submission metadata constraint

The first `git add` of the implementation and evidence returned exit 128:

```text
fatal: Unable to create '/mnt/dev-ssd/workspaces/symphony/GH-228/.git/index.lock': Read-only file system
```

The assigned checkout's original Git metadata is read-only. Publication therefore
uses separate metadata inside this same workspace, without another working tree:

```sh
git clone --bare --no-hardlinks --single-branch --branch symphony/GH-228 . target/gh-228/publish.git
git --git-dir=target/gh-228/publish.git remote set-url origin https://github.com/musutrade/Harness-Gate.git
git --git-dir=target/gh-228/publish.git read-tree HEAD
```

All three commands exited 0. The bare index needed initialization from HEAD
before staging the scoped edits; review caught the empty initial index. Staging and committing use that `--git-dir` with
`--work-tree` set to the assigned checkout; pushing uses the same assigned branch.
The original `.git` and delivery-base references remain unchanged. A future controller
handoff must record the actual remote pushed commit from the publication metadata;
the original read-only metadata continues to report the preserved predecessor
commit. This is a submission-environment limitation, not a validation pass.

A direct submission-time docs check without the runner's command-local environment
returned exit 1: generated examples/schema and migration subprocess checks failed.
The checker suppresses their stderr, so no more specific subprocess cause is
asserted. This invocation omitted the recorded Cargo and scratch environment. Its original JSON is retained in
[submission-docs-missing-environment.json](https://github.com/musutrade/Harness-Gate/blob/d9a932b23dcfea709175285b594ca200450a9971/docs/quality/gh-228/submission-docs-missing-environment.json).
The runner was then used again with its recorded `CARGO_TARGET_DIR` and scratch
environment; docs, strict OpenSpec and diff checks each exited 0 again. Their
command records and logs are retained under `submission-checks/`.

## Push blocked

[submission-blocker.json](https://github.com/musutrade/Harness-Gate/blob/d9a932b23dcfea709175285b594ca200450a9971/docs/quality/gh-228/submission-blocker.json) retains both exact push commands,
exit 128 results and errors: the configured proxy could not connect to GitHub,
and a direct attempt could not resolve `github.com`. Implementation and local
validation are complete, but no push, PR, CI acceptance or completion declaration
is claimed. The original `.git` still reports the accepted predecessor; use the
workspace-local `target/gh-228/publish.git` metadata for the completed commit.

Recovery must preserve this workspace and its original capture/runtime artifacts.
Once GitHub push connectivity is available, resume the same branch with:

```sh
git --git-dir=target/gh-228/publish.git push origin refs/heads/symphony/GH-228:refs/heads/symphony/GH-228
```

Then create the PR targeting `main` using the prepared `target/gh-228/pr-body.md`,
record its actual pushed SHA/number in the runtime handoff, and stop for the
controller. Do not rerun implementation or discard the retained failures.
