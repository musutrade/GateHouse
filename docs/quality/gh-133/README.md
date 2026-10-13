# GH-133: real Angular/Rust contract gates

Scope: [OpenSpec task 3.3](../../../openspec/changes/typescript-angular-reference-adapter/tasks.md),
following [ADR-0040](../../adr/0040-language-agnostic-evidence-policy.md).
The [native collector](https://github.com/musutrade/Harness-Gate/blob/8d93ea53c7cf8b314bda816eafe13209da25c742/tools/quality/collect_typescript_contracts.py)
runs the existing Angular fixture against its live Rust provider in disposable
copies. The [adapter](https://github.com/musutrade/Harness-Gate/blob/8d93ea53c7cf8b314bda816eafe13209da25c742/tools/quality/typescript_contracts.py) parses actual
oasdiff output and compares complete freshly generated client trees, then emits
the existing evidence, project and policy contracts. No generic policy, report,
collector protocol or schema implementation changes.

| Native scenario | Provider local | Consumer local | OpenAPI breaks | Client drift | Project |
| --- | --- | --- | --- | --- | --- |
| Unchanged contract, current checked-in client | pass | pass | 0 | false | pass |
| Response `currency` becomes optional; checked-in client remains required | pass | pass | 1 | true | fail |

Both scenarios perform locked npm installation, Angular production build, Rust
build and LLVM coverage tests, and all 7 Angular tests with the live provider.
The provider serves the measured OpenAPI bytes at `/openapi.json`. It continues
returning `currency: "USD"`, so its local response tests and the consumer's actual
HTTP test pass even after the schema guarantee is removed. oasdiff reports
`response-property-became-optional`; real regeneration changes
`currency: Quote.currency` to `currency?: Quote.currency`. Raw function counters
for `provider/src/main.rs` (2/3) and `app/src/app/quote-client.ts` (1/1) pass the
explicit diagnostic local threshold of 1/2. These two bounded file gates do not
certify the entire components' coverage.

The breaking project has three required blockers: `contract.breaking_changes`,
`contract.client_drift` and `contract.compatible`. Each lossless report gate links
the `quote-client` relationship, `api` provider, `frontend` consumer, original
contract/provider/consumer subject identities, baseline and generation-input
digests, raw oasdiff JSON, generated trees, native counters and invocation logs.
Tool versions participate in series identity; exact argv, cwd, relevant environment,
exit statuses and stream digests remain in the linked manifest and receipt.
Changing diagnostic policy limits to accept the same measured facts passes,
demonstrating that the tool parser supplies facts and generic policy decides.

## Retained evidence and reproduction

[index.json](https://github.com/musutrade/Harness-Gate/blob/d9a932b23dcfea709175285b594ca200450a9971/docs/quality/gh-133/index.json) pins both native archives, their complete-file receipts,
explicit run/commit/base/target contexts and [reports.tar.gz](https://github.com/musutrade/Harness-Gate/blob/d9a932b23dcfea709175285b594ca200450a9971/docs/quality/gh-133/reports.tar.gz).
The reports archive contains project, policy, normalized evidence, context and
full project report JSON for each scenario. Raw artifacts are retained in
[compatible-native.tar.gz](https://github.com/musutrade/Harness-Gate/blob/d9a932b23dcfea709175285b594ca200450a9971/docs/quality/gh-133/compatible-native.tar.gz) and
[breaking-native.tar.gz](https://github.com/musutrade/Harness-Gate/blob/d9a932b23dcfea709175285b594ca200450a9971/docs/quality/gh-133/breaking-native.tar.gz); source, lockfiles, both generated
trees and all command outputs are included. Dependency caches and build outputs
are removed after collection. Absolute invocation paths are historical provenance;
offline replay uses the extracted artifact root.

Run the retained integration and adversarial cases without Node or oasdiff:

```bash
python3 -m unittest discover -s tools/quality/tests -p test_typescript_contracts.py -v
```

For fresh native reproduction use the fixture's pinned Node 24.18.0, npm 11.16.0,
openapi-typescript-codegen 0.29.0, plus cargo-llvm-cov 0.9.0 and Rust with the LLVM
tools component. The retained Rust toolchain is 1.97.1. Obtain
[oasdiff v1.11.7 Linux x86_64](https://github.com/oasdiff/oasdiff/releases/tag/v1.11.7),
whose `oasdiff_1.11.7_linux_amd64.tar.gz` release archive SHA-256 is
`97f1052365f74e6fd6f4d8fa108606e09391aebb8ecbf3b5e7a4059d54327224` and executable
SHA-256 is `0f3f70ea55dc50b8cae7e495f26f1bfc7e9eded1114990241f0b00f89950ba55`.
The collector checks the executable digest before running it. The original
release checksums are included in the validation archive.

```bash
python3 tools/quality/collect_typescript_contracts.py \
  --oasdiff target/quality/gh133/bin/oasdiff \
  --scenario compatible --output target/quality/gh133/native-compatible-fresh
python3 tools/quality/collect_typescript_contracts.py \
  --oasdiff target/quality/gh133/bin/oasdiff \
  --scenario breaking --output target/quality/gh133/native-breaking-fresh
```

Output directories must be new. For a retained CLI replay, extract one native
archive to a directory, extract `reports.tar.gz` separately, and supply the
matching receipt SHA from `index.json` and scenario context JSON:

```bash
python3 tools/quality/typescript_contracts.py --native PATH_TO_EXTRACTED_NATIVE \
  --receipt-sha256 SHA_FROM_INDEX --expected PATH_TO_SCENARIO_CONTEXT_JSON \
  --output target/quality/gh133/replayed-report.json
```

Exit 0 means project pass; exit 1 means a blocking result or measurement error.
The output belongs outside the native directory, whose full inventory is pinned.
The caller must pin a newly collected receipt and construct its explicit context
from the retained revision, `run=gh133-SCENARIO`, `target=native-contract`, with
`base_commit=commit`. The two scenarios are fresh disposable variants of one
checkout revision; their distinct source digests bind the change. They are not
accepted Git base/head history pairs for task 4.1. Receipts provide integrity
relative to the caller's trusted digest, not signatures or release authority.

## Fail-closed behavior and bounded capability

The integration suite rejects failed/absent invocations, missing or tampered raw
files, extra files, invalid tool output, changed versions or executable digests,
wrong generation inputs or live-provider environment, stale/modified client
provenance, missing generation output, caller-context mismatch, unavailable
normalized measurements and generic provider/consumer/baseline/artifact binding
mismatches. Missing oasdiff output cannot become zero breaks. Native failures
leave a failed manifest; replay refuses it. The breaking contract remains a
successful *measurement* whose facts fail required policy.

**TS-04 — byte drift versus generation-input freshness:** generic v1 requires
`contract.client_drift` to agree with the checked-in client's input-contract
digest versus the head contract digest. This fixture authenticates its checked-in
client against fresh baseline generation and compares that full tree to fresh
head generation. If bytes and input freshness disagree, normalization fails
closed instead of altering generic meaning. A whitespace-only baseline change
with identical generated bytes reproduces this boundary in the suite. Such
cases and arbitrary client migration workflows are outside this bounded adapter;
support would need further adapter evidence or a separately reviewed generic
contract change. TS-04 is disposed by narrowing capability, not certified as a
supported unchanged-byte migration.

This is development-only shadow evidence. It does not certify browser E2E, SSR,
accessibility, mutation, security, templates, route coverage, or generated-code
coverage. Tasks 4.1–4.3 (acceptance pairs, advisory CI, certification and review)
remain open; existing required Rust authority is unchanged.

## Validation

Full logs and exit statuses are retained in [validation.tar.gz](https://github.com/musutrade/Harness-Gate/blob/d9a932b23dcfea709175285b594ca200450a9971/docs/quality/gh-133/validation.tar.gz).
Rust checks use `CARGO_TARGET_DIR=$PWD/target` inside this workspace. Required
hosted CI is pending and must pass before merge.

| Command | Actual result |
| --- | --- |
| `cargo nextest run --manifest-path tools/harness-gate/Cargo.toml --locked` | Exit 0; 315 passed, 0 skipped |
| `cargo fmt --manifest-path tools/harness-gate/Cargo.toml -- --check` | Exit 0 |
| `cargo clippy --manifest-path tools/harness-gate/Cargo.toml --all-targets -- -D warnings` | Exit 0 |
| `python3 -m unittest discover -s tools/quality/tests -v` | Exit 0; 261 passed, including 11 contract integration tests |
| `python3 tools/quality/docs_consistency.py --output target/quality/docs-consistency.json` | Exit 0; status `pass`, with workspace-local `CARGO_TARGET_DIR` |
| `python3 -m unittest discover -s tools/quality/tests -p test_project_report.py -v` | Baseline: exit 0; 12 passed |

`harness-gate config check` and `harness-gate verify --profile ci --all` are **not
applicable**: this checkout has no `.harness-gate/flow.toml` with a declared `ci`
profile. No project-local configuration was invented.

The first documentation check exited 1 because the default Cargo configuration
points outside the writable workspace. A direct diagnostic command,
`cargo run --manifest-path tools/harness-gate/Cargo.toml --locked -- --version`,
exited 101: `error: failed to open: /home/gem/cargo-target/debug/.cargo-build-lock`,
`Read-only file system (os error 30)`. Setting `CARGO_TARGET_DIR=$PWD/target`
allowed the complete documentation check to pass. The original report and
diagnostic error are retained alongside the successful result.

An initial native attempt failed with `FileNotFoundError` for
`target/quality/gh133/native-compatible/cargo-target/debug/reference-provider`
(and the equivalent breaking path). The collector now uses the package's actual
`angular-reference-provider` executable. Both final native invocations succeeded;
the initial failure logs remain in the validation archive.

`git add tools/quality/collect_typescript_contracts.py` exited 128 because this
workspace's `.git/index.lock` is read-only. Its Git metadata was copied entirely
within this workspace to ignored `target/quality/gh133/delivery.git`, with no
symlinks or external object directories, for committing and pushing the same
branch and working tree. Original metadata stays at the baseline; the handoff
declares the actual pushed SHA. See `delivery-preparation.log` in the validation
archive.
