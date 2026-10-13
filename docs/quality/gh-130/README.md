# GH-130 validation evidence

Scope: [OpenSpec tasks 2.1–2.2](../../../openspec/changes/typescript-angular-reference-adapter/tasks.md).
The [source semantics decision](../typescript-source-semantics.md) resolves
TS-01/TS-02 within original TypeScript file/function/method coverage.
[ADR-0040](../../adr/0040-language-agnostic-evidence-policy.md) and
the generic subject/series contracts are unchanged; no generic amendment is needed.
Collector/policy integration, TS-03 and certification remain future tasks.

The focused replay suite passes 18 tests, including exact native counters,
distinct same-named methods, anonymous arrows, malformed/stale/tampered/incomplete
provenance, empty denominators and incompatible TypeScript/Rust series.
Native build/test collection is reused from GH-129; GH-130 regenerates the pinned
parser inventory and native counter oracle without changing the measured sources.

| Command | Actual result |
| --- | --- |
| `cargo nextest run --manifest-path tools/harness-gate/Cargo.toml --locked` | Exit 0; 315 passed, 0 skipped |
| `cargo fmt --manifest-path tools/harness-gate/Cargo.toml -- --check` | Exit 0 |
| `cargo clippy --manifest-path tools/harness-gate/Cargo.toml --all-targets -- -D warnings` | Exit 0 |
| `python3 -m unittest discover -s tools/quality/tests -v` | Exit 0; 230 tests passed |
| `python3 tools/quality/docs_consistency.py --output target/quality/docs-consistency.json` | Exit 0; status `pass` |
| `python3 -m unittest discover -s tools/quality/tests -p test_typescript_semantics.py -v` | Exit 0; 18 tests passed |
| Pinned compiler inventory and native Istanbul oracle regeneration | Both exit 0; outputs byte-identical to committed fixtures |
| `npm ci --no-audit --no-fund --cache /tmp/gh130-npm-cache` in fixture `app` | Exit 0 |

[validation.tar.gz](https://github.com/musutrade/Harness-Gate/blob/d9a932b23dcfea709175285b594ca200450a9971/docs/quality/gh-130/validation.tar.gz) retains full logs, command/status records,
the docs report, regeneration outputs and all 11 normalized subjects from five
original TypeScript files. [index.json](https://github.com/musutrade/Harness-Gate/blob/d9a932b23dcfea709175285b594ca200450a9971/docs/quality/gh-130/index.json) records archive size/digest
and fixture/generator digests. Regeneration commands are in the
[semantic fixture README](../../../tools/quality/fixtures/typescript-angular/semantics/README.md).
Hosted required CI must pass before merge and is pending at submission.

`harness-gate config check` and `harness-gate verify --profile ci --all` are **not
applicable**: this checkout has no `.harness-gate/flow.toml`, hence no declared
project-local `ci` profile. No substitute configuration was created.

Validation uses `CARGO_TARGET_DIR` inside this workspace because the inherited
shared target directory is outside the writable sandbox. Dependency installation
uses `/tmp/gh130-npm-cache`. No source checkout or other workspace is accessed.

`git add` failed with exit 128: `Unable to create
'/home/gem/symphony-workspaces/GH-130/.git/index.lock': Read-only file system`.
The exact command/error is retained in `delivery-preparation.log`. This workspace's
Git metadata was copied to ignored `target/quality/gh130/delivery.git` to commit
and push the same branch and working tree. The original read-only metadata remains
at its initial revision; the runtime handoff names the actual pushed commit.
