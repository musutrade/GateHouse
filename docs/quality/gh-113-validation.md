# GH-113 validation record

Scope: OpenSpec `language-agnostic-evidence-policy-architecture` tasks 4.1–4.4.
The [collector contract](collector-protocol.md) records protocol, runner, typed
failures and transport-independent normalized policy input. Existing Rust
collection, CI authority and ADR-0039 are unchanged. Generic policy and real
Rust migration remain later tasks.

Before implementation, the collector runner was absent; the existing normalized
evidence suite passed 18 tests. The new focused suite passes 12 tests, including
17 declarative scenarios (12 through both transports and 5 subprocess-only),
four synthetic component projections and additional trust-boundary subcases.
GH-112 was confirmed closed before implementation; its normalized contracts are
in the prepared branch baseline `8d26e56` (PR #121).

## Local validation

| Command | Result |
| --- | --- |
| `cargo nextest run --manifest-path tools/harness-gate/Cargo.toml --locked` | Initial exit 101; workspace-target rerun passed 315 tests, 0 skipped |
| `cargo fmt --manifest-path tools/harness-gate/Cargo.toml -- --check` | Passed |
| `cargo clippy --manifest-path tools/harness-gate/Cargo.toml --all-targets -- -D warnings` | Initial exit 101; workspace-target rerun passed |
| `python3 -m unittest discover -s tools/quality/tests -v` | Passed 158 tests |
| `python3 tools/quality/docs_consistency.py --output target/quality/docs-consistency.json` | Initial exit 1; workspace-target rerun passed |
| `harness-gate config check` | Not applicable: no `.harness-gate/flow.toml` |
| `harness-gate verify --profile ci --all` | Not applicable: no project configuration or declared `ci` profile |

The initial Nextest and Clippy commands failed before compilation with:

```text
error: failed to open: /home/gem/cargo-target/debug/.cargo-build-lock
Caused by:
  Read-only file system (os error 30)
```

Their reruns changed only the environment to
`CARGO_TARGET_DIR=/home/gem/symphony-workspaces/GH-113/target/gh-113-cargo`.
The initial docs command reported
`quality gate failed: documentation, examples, or schema synchronization failed`;
its report shows failed Cargo-backed examples/migration and unsynchronized schema,
with no link failures. The same workspace-target override made it pass.
No generic project-local configuration was invented.

[Machine validation summary](https://github.com/musutrade/Harness-Gate/blob/d9a932b23dcfea709175285b594ca200450a9971/docs/quality/gh-113/validation-summary.json) records commands,
exit codes, durations, tested source digests and not-applicable decisions.
[Retained full logs/reports](https://github.com/musutrade/Harness-Gate/blob/d9a932b23dcfea709175285b594ca200450a9971/docs/quality/gh-113/local-evidence.tar.gz) contain baseline,
focused and full suites, initial failures, successful reruns and docs reports.
Reports identify the pre-commit checkout base; source digests bind the modified
implementation that was tested. Only OpenSpec tasks 4.1–4.4 are checked off here.

The initial `git add tools/quality/collector_runner.py` exited 128 with
`fatal: Unable to create '/home/gem/symphony-workspaces/GH-113/.git/index.lock': Read-only file system`.
Delivery uses a copy of this workspace's Git metadata at `target/gh-113-git`, with
`--git-dir=target/gh-113-git --work-tree=.`. This preserves the prepared branch and
base while committing the current workspace files. The original read-only `.git`
stays at the checkout base; the handoff declares the actual pushed branch/SHA.
No source checkout or other workspace was accessed.

Required CI remains pending. Task checkmarks attest local contract validation,
not full architecture acceptance or permission to replace existing Rust authority.
