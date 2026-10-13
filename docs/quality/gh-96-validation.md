# GH-96 required quality gates: validation record

Scope: OpenSpec tasks 8.1–8.4 only. This integrates the independently developed
GH-91–GH-95 gates; it does not accept a replacement baseline or complete the
remaining OpenSpec acceptance tasks. See [ADR-0039](../adr/0039-required-risk-and-traceability-gates.md)
and the [local command contract](../../tools/quality/README.md).

## Candidate identity and reproduction

The development candidate compares base
`c0e612351c2efd515fde96bcefad80c1365534a2` with head
`41e23761e0caa06cab1f6d5ebca1a79648fbb301`, run `gh96-local-3`.
Both snapshots were independently archived and measured with the same GH-94 v2
analyzer and instrumentation rules. Subsequent commits contain documentation
and retained evidence only; CI must collect again at the submitted commit.

```sh
python3 tools/quality/ci_quality.py collect \
  --base-sha c0e612351c2efd515fde96bcefad80c1365534a2 \
  --head-sha 41e23761e0caa06cab1f6d5ebca1a79648fbb301 \
  --run-id gh96-local-3 --output target/quality/gh96-candidate-3
python3 tools/quality/ci_quality.py verify \
  --base-sha c0e612351c2efd515fde96bcefad80c1365534a2 \
  --head-sha 41e23761e0caa06cab1f6d5ebca1a79648fbb301 \
  --run-id gh96-local-3 --output target/quality/gh96-candidate-3
```

Collection requires checking out the measured head and a fresh output directory.
The workspace's Git metadata is read-only. Local commit/collection commands used
a copy of that metadata under this workspace's ignored `target/gh96-git`, with
a Git wrapper scoped to this working directory. No other checkout was accessed.
The ambient shared Cargo target was also read-only; standalone Rust checks used
`CARGO_TARGET_DIR="$PWD/target/rust"`. Collection creates its own target directories.

## Behavior confirmed before adoption

The original six-module gate passed. The first extended candidate correctly
failed app coverage at 300/390 executable lines (76.92%) and project coverage at
333/422 (78.91%). Two CLI integration tests now exercise comparison differences,
canary/rollback evidence, nested project discovery, explicit configuration and
invalid project roots. They change no product behavior or threshold.

The first snapshot collection exposed omitted repository-relative adapter
fixtures; snapshot archives now include `tools/quality/fixtures`. The next run
passed coverage and risk but correctly rejected a stale matrix source binding
after tests were appended to a bound file. Moving the new tests to
`ci_adoption_test.rs` restored the bound file byte for byte. No matrix binding,
mandatory path, policy or historical baseline was weakened.

## Gate results and collection cost

The [candidate manifest](https://github.com/musutrade/Harness-Gate/blob/d9a932b23dcfea709175285b594ca200450a9971/docs/quality/gh-96/candidate.json) records all four stages as
successful and hashes **137 artifacts**. The [complete raw archive](https://github.com/musutrade/Harness-Gate/blob/d9a932b23dcfea709175285b594ca200450a9971/docs/quality/gh-96/candidate.tar.gz)
contains those artifacts and the manifest, including original base/head source
archives, LLVM JSON/LCOV/Cobertura, source manifests, exact risk counters,
isolated matrix evidence and command logs. Extracting it into a fresh directory
and running `ci_quality.py verify` returned 0. Archive SHA-256:
`e227afc1ee994a2889d4df09758c339472d306aec4e3f2371ae1958db1c28dfd`.

| Required stage | Result | Local wall time |
| --- | --- | ---: |
| Original six-module coverage | All six and aggregate pass; 9,749/11,309 lines (86.21%) | 55.13 s |
| Extended production coverage | All ten and aggregate pass; 9,844/11,470 lines (85.82%) | 2.18 s |
| Independent base/head risk | 212 identities, all 32 selected responsibilities pass, zero comparison failures | 67.78 s |
| Isolated critical paths | 12/12 applicable rows, including all six mandatory paths | 213.20 s |

Total collection: **338.72 seconds** on local Linux x86_64, 8 logical CPUs,
Rust/Cargo 1.97.1, Python 3.14.4, cargo-llvm-cov 0.9.0 and nextest 0.9.143.
Compared with the six-module stage in this same run, the additional stages and
orchestration cost 283.59 seconds; total collection is about 6.14 times the
legacy stage alone.
This is one local reproduction of the CI command, including compilation but
excluding tool installation and hosted runner scheduling. Some standalone checks
overlapped matrix collection, so these are observed wall times, not an isolated
microbenchmark. CI records its own stage times in every uploaded manifest.

[Production counts](https://github.com/musutrade/Harness-Gate/blob/d9a932b23dcfea709175285b594ca200450a9971/docs/quality/gh-96/production.json) preserve different line, function and
region denominators. App is 317/390 lines (81.28%); project is 341/422 (80.81%).
Both margins are narrow and remain blocking at the unchanged 80% threshold.
[Legacy counts](https://github.com/musutrade/Harness-Gate/blob/d9a932b23dcfea709175285b594ca200450a9971/docs/quality/gh-96/coverage.json) retain the original six-module calculation.
[Risk comparison](https://github.com/musutrade/Harness-Gate/blob/d9a932b23dcfea709175285b594ca200450a9971/docs/quality/gh-96/risk.json) uses exact rational CRAP for decisions, with
75 unmodified nonselected debt identities retained in archived `head-risk.json`.
Risk scope remains six files; branch coverage is unsupported. These results do
not claim repository-wide risk coverage or rounded-threshold acceptance.
[Matrix evidence](https://github.com/musutrade/Harness-Gate/blob/d9a932b23dcfea709175285b594ca200450a9971/docs/quality/gh-96/critical-paths.json) is Linux-only and does not claim
real Docker/Podman execution. [Initial failures](https://github.com/musutrade/Harness-Gate/blob/d9a932b23dcfea709175285b594ca200450a9971/docs/quality/gh-96/initial-failures.json)
retain the unsuccessful candidate identities and stage diagnostics.

## Existing performance fixture

The [five-sample report](https://github.com/musutrade/Harness-Gate/blob/d9a932b23dcfea709175285b594ca200450a9971/docs/quality/gh-96/benchmarks.json) records serial median verification
at **0.972 s** and parallel median at **0.644 s**, a **33.73% reduction**.
Its unchanged policy compares parallel versus serial median, allowing at most
a 15% increase; `regression` is false. All scope samples report equivalent
results. The five warm test-suite samples have a 12.131 s median; the tool's
initial sample is 11.932 s. The target already held debug builds, so the latter
is not a claim of a physically cold cache despite the legacy `cold` field name.

The uninstrumented release-small binary is 8,711,168 bytes, SHA-256
`3efaf9115eebf7a75e2f0f8ffeb6a44a2ffeed6b065fa9d96b8bc6137b0f0c5b`.
The fixture and product source are unchanged. These numbers describe this local
series, not a cross-machine comparison against ADR-0025. Raw per-sample reports,
logs and invocation records are retained in [validation evidence](https://github.com/musutrade/Harness-Gate/blob/d9a932b23dcfea709175285b594ca200450a9971/docs/quality/gh-96/validation.tar.gz).

## Validation commands

The following commands use the same schema and policy as CI:

```sh
cargo nextest run --manifest-path tools/harness-gate/Cargo.toml --locked
cargo fmt --manifest-path tools/harness-gate/Cargo.toml -- --check
cargo clippy --manifest-path tools/harness-gate/Cargo.toml --all-targets -- -D warnings
python3 -m unittest discover -s tools/quality/tests -v
python3 -m py_compile tools/quality/*.py tools/quality/tests/*.py
python3 tools/quality/docs_consistency.py --output target/quality/docs-consistency.json
openspec validate strict-json-results-and-risk-based-quality-gates --strict
python3 tools/quality/benchmarks.py --output target/quality/gh-96/benchmarks.json \
  --samples 5 --raw-dir target/quality/gh-96/benchmark-runs
```

`harness-gate config check` and `harness-gate verify --profile ci --all` are **not
applicable**: this source checkout has no `.harness-gate/flow.toml` declaring a
`ci` profile. No configuration was invented. Linux fixture execution does not
certify macOS/Windows execution or real container adapters. Hosted required
checks and final acceptance remain the controller's responsibility.

Negative tests reject every applicable job's failure, cancellation, skip,
missing result and unknown result; only explicitly push-only jobs may skip on
PRs. Candidate tests reject stale head/base/run identities, changed or missing
raw artifacts, path escapes, and incomplete/unsuccessful stages. Collection
continues independent stages after an ordinary failure and retains raw logs.
The workflow's `always()` upload and aggregate wiring are checked by the same
quality-tool suite.

Final results: **315 Rust tests passed, zero skipped; 119 quality-tool tests
passed; fmt, Clippy, bytecode compilation, documentation/schema consistency and
strict OpenSpec validation passed.** Collection, extracted-archive verification
and the benchmark command all returned 0. Exact commands, results and local
environment limitations are recorded in [validation.json](https://github.com/musutrade/Harness-Gate/blob/d9a932b23dcfea709175285b594ca200450a9971/docs/quality/gh-96/validation.json);
complete logs are in the validation archive. Tasks 8.1–8.4 are locally validated;
tasks 9.1–9.4 and full-change acceptance remain unchecked.
