# GH-131 validation evidence

Scope: [OpenSpec task 3.1](../../../openspec/changes/typescript-angular-reference-adapter/tasks.md).
The [versioned TypeScript/Angular collector](../typescript-collector.md) normalizes
the retained GH-129 frontend artifacts and GH-130 parser inventory through both
existing generic runner transports. TS-03 is resolved with explicit capability
states on every requested subject, without changing generic request/response,
project, evidence, policy or aggregation contracts. This follows
[ADR-0040](../../adr/0040-language-agnostic-evidence-policy.md).

The baseline source-semantics suite passed all 18 tests before implementation.
The new 11-test adapter suite covers mixed file/function/method/route scope,
every generic capability, exact native counters across five original files,
zero denominators, unavailable template/generated sources, stale and tampered
inputs, configuration drift, source-map failure, missing/undeclared artifacts,
request and response provenance, subprocess exit, timeout, malformed stdout and
rejection of final decision fields. A saved request reproduces 14 normalized
records and 22 numeric metrics against the unchanged retained fixture.

| Command | Actual result |
| --- | --- |
| `cargo nextest run --manifest-path tools/harness-gate/Cargo.toml --locked` | Exit 0; 315 passed, 0 skipped |
| `cargo fmt --manifest-path tools/harness-gate/Cargo.toml -- --check` | Exit 0 |
| `cargo clippy --manifest-path tools/harness-gate/Cargo.toml --all-targets -- -D warnings` | Exit 0 |
| `python3 -m unittest discover -s tools/quality/tests -v` | Exit 0; 241 tests passed |
| `python3 tools/quality/docs_consistency.py --output target/quality/docs-consistency.json` | Exit 0; status `pass` with workspace-local Cargo target |
| `python3 -m unittest discover -s tools/quality/tests -p test_typescript_reference.py -v` | Exit 0; 11 tests passed |
| `python3 -m unittest discover -s tools/quality/tests -p test_typescript_semantics.py -v` | Exit 0; 18 baseline tests passed |

[validation.tar.gz](https://github.com/musutrade/Harness-Gate/blob/d9a932b23dcfea709175285b594ca200450a9971/docs/quality/gh-131/validation.tar.gz) retains logs, command/status records,
the docs report, replay request/project and normalized evidence.
[index.json](https://github.com/musutrade/Harness-Gate/blob/d9a932b23dcfea709175285b594ca200450a9971/docs/quality/gh-131/index.json) pins the archive and relevant adapter/fixture digests.
To replay, extract the archive, set the saved request's `workspace_root` to
`tools/quality/fixtures/typescript-angular` as an absolute path and `output_root`
to a new empty directory, then invoke the generic runner as documented in the
collector guide. Those roots are caller-local and are not part of the receipt
binding. The native archive and parser index remain at their repository paths;
the replay recreates the source-bound output artifact copies.

The original tool revision and archive remain intact; the receipt explicitly
binds retained replay to a new context and requires exact current source/config
bytes. This is not a new native tool execution. Per-source archive duplication
is a deliberate storage cost of the existing artifact provenance contract.
Policy/ratchets, OpenAPI integration, acceptance pairs and certification remain
tasks 3.2–4.3. Required hosted CI is pending and must pass before merge.

`harness-gate config check` and `harness-gate verify --profile ci --all` are **not
applicable**: this checkout has no `.harness-gate/flow.toml` and no declared
project-local `ci` profile. No substitute configuration was created.
Rust validation and the Cargo-backed documentation check set `CARGO_TARGET_DIR`
to this workspace's `target` directory. The initial documentation command exited
1 with `quality gate failed: documentation, examples, or schema synchronization failed`:
the evidence archive/index links did not exist yet and Cargo-backed checks failed
under the inherited target configuration. After creating the evidence files and
using the workspace-local target, all documentation checks passed. Both reports
are retained; the initial failure is not counted as a pass.
No native dependency installation or source checkout/other workspace access
was needed.

`git add tools/quality/typescript_reference.py` failed with exit 128:
`Unable to create '/home/gem/symphony-workspaces/GH-131/.git/index.lock': Read-only file system`.
The exact command/error is retained in `delivery-preparation.log`. This workspace's
Git metadata was copied to ignored `target/quality/gh131/delivery.git` to commit
and push the same branch and working tree. The original read-only metadata remains
at its initial revision; the runtime handoff names the actual pushed commit.
