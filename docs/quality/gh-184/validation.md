# GH-184 validation

Scope: OpenSpec `integrate-generic-quality-into-project-workflow`, tasks 6.1–6.4.
See [ADR-0046](../../adr/0046-capability-driven-quality-profiles.md) and the
[profile contract](../../quality-profiles.md). Whole-change and hosted acceptance
remain pending; preset generation and workflow wiring belong to sections 7–8.

## Behavior and evidence

Complete assurance is the generic default and requires every configured required
policy binding and its compatible producer. Partial assurance permits explicitly
selected omissions, reported as `not_collected`; passing selected policy never
becomes full-quality PASS. Empty profiles launch no collector and invoke no
project evaluator. Custom profile names have identical semantics.

The `unknown-profiles.json` fixture declares cheap `bundle.size` and expensive
`risk.crap` capabilities for `nebula-unregistered-2049`, with hook/full/ci and
custom selections. Configuration and signed collector tests exercise metadata,
omissions, selected-policy failure, and retained collection. The architecture
guard includes all generic profile/configuration and verification modules and
rejects a closed ecosystem switch, including an arbitrary future ecosystem.

`certified-rust-profiles.json` binds the original retained Rust reference line and
region coverage limits (4/5) and CRAP limit (30), requiredness and entire original
series. Tests compare these to a fresh projection of the pinned historical native
archive, and validate both default-complete profiles. Full/ci evaluation rejects
CRAP 40 with baseline 35 and limit 30; unavailable states retain no metric value.
The native TypeScript CRAP-only request preserves its existing series and other
certified capabilities while CRAP remains explicitly unsupported and valueless.

CI reuse tests first collect authenticated evidence, pin the returned envelope
and raw artifacts, remove the executable, and replay through collection and
verify. Evidence, response and decisions remain identical, with producer origin
`retained`; the consumed nonce cannot be replayed. A mixed-producer case reuses
one response and launches only the missing producer. Digest, binding, schema,
run, series, subject, raw artifact, missing file, inactive producer and changed
source negative controls fail before fallback collection.

## Local validation

Logs reside in `target/quality/gh-184/`. Cargo-dependent required commands use
`CARGO_TARGET_DIR=$PWD/target`; the environment default `/home/gem/cargo-target`
is read-only. The initial focused nextest attempt failed with `Read-only file
system (os error 30)` there; rerunning with a workspace target resolves it.

| Command | Actual result | Log |
| --- | --- | --- |
| `cargo nextest run --manifest-path tools/harness-gate/Cargo.toml --locked` | 373 passed, 0 skipped | `nextest.log` |
| `cargo fmt --manifest-path tools/harness-gate/Cargo.toml -- --check` | Passed | `fmt.log` |
| `cargo clippy --manifest-path tools/harness-gate/Cargo.toml --all-targets -- -D warnings` | Passed | `clippy.log` |
| `python3 -m unittest discover -s tools/quality/tests -v` | 335 passed | `python-tests.log` |
| `python3 tools/quality/docs_consistency.py --output target/quality/docs-consistency.json` | Passed | `docs-consistency.log` |
| `openspec validate integrate-generic-quality-into-project-workflow --strict` | Passed | `openspec.log` |

The initial Python run caught a stale declaration hash and an overly broad
TypeScript assertion. Refreshing the verified declaration hash and checking the
CRAP capability by name (while retaining the other native capabilities) resolved
both. A subsequent documentation test observed the evidence link before this
file was written; the completed file and five documentation tests passed.
The final full-suite result above supersedes these intermediate attempts.

Production coverage passes every blocking boundary at the unchanged threshold;
the [coverage summary](https://github.com/musutrade/Harness-Gate/blob/d9a932b23dcfea709175285b594ca200450a9971/docs/quality/gh-184/coverage-summary.json) retains exact aggregate and boundary
counts. The independent instrumented run passed all 373 tests. Commands below
all passed, with `CARGO_TARGET_DIR=$PWD/target/gh184-coverage-final`,
`CARGO_INCREMENTAL=0`, `CARGO_PROFILE_DEV_DEBUG=0` and
`CARGO_PROFILE_TEST_DEBUG=0` for cargo:

```bash
cargo llvm-cov nextest --package harness-gate --manifest-path tools/harness-gate/Cargo.toml --locked --test-threads 4 --no-fail-fast --json --output-path target/quality/gh-184/coverage.raw.json
cargo llvm-cov report --package harness-gate --manifest-path tools/harness-gate/Cargo.toml --lcov --output-path target/quality/gh-184/coverage.lcov
cargo llvm-cov report --package harness-gate --manifest-path tools/harness-gate/Cargo.toml --cobertura --output-path target/quality/gh-184/coverage.cobertura.xml
python3 tools/quality/coverage.py --production --raw target/quality/gh-184/coverage.raw.json --lcov target/quality/gh-184/coverage.lcov --output target/quality/gh-184/production.json
```

Logs: `coverage-final.log`, `coverage-export.log`, `coverage-production.log`.
An earlier instrumentation attempt was interrupted; overlapping cleanup produced
`No such file or directory` when starting removed test executables, with a
mixed-output log. The final run used a separate target after that attempt and
is the authoritative result. Hosted source-risk comparison remains pending;
local coverage does not establish Required Quality Aggregate status.

The source-inventory declaration hash was refreshed after the AST tool confirmed
`config/quality/model.rs` has zero handwritten executable symbols. No production
file was excluded, no measurement selection changed, and no threshold changed.
AST complexity checks drove extraction of request preparation and retained
validation; all measured functions in changed production modules are at most 23.
The [AST summary](https://github.com/musutrade/Harness-Gate/blob/d9a932b23dcfea709175285b594ca200450a9971/docs/quality/gh-184/complexity-summary.json) retains symbol counts and maxima.

`harness-gate config check` and `harness-gate verify --profile ci --all` are
**not applicable**, not passed: this checkout has no `.harness-gate/flow.toml`
declaring `ci`. No synthetic configuration was added. Required Quality Aggregate
is **CI pending**; the controller owns hosted checks and acceptance.

## CI topology and cost boundary

The [accepted model](../ci-topology/README.md) remains unchanged: Quality Coverage
and Critical Paths owns authoritative coverage/risk/CRAP collection, the shadow
consumer validates retained evidence, and Required Quality Aggregate evaluates
only dependencies. This issue changes no workflow, runner matrix, collection
command, upload/download topology, or required job. It adds no hosted collection
or job. Tests prove same-binding reuse launches zero equivalent collectors and
mixed reuse launches only missing producers.

Additional profile and retention tests add validation work inside existing jobs;
there is no hosted timing sample for this unpushed change, so no runner-minute,
critical-path or latency improvement is claimed. The existing receipt validation,
I/O and new tests have unmeasured hosted runtime. Section 8 owns hosted wiring
and timing/runner-work capture under the accepted event/runner model. No local
timing is substituted for hosted evidence and no cache/build-once experiment is
reintroduced.

## Publication

The workspace `.git` is mounted read-only. Publication uses a metadata copy under
this workspace's ignored `target/` directory with the same prepared branch and
parent; the validated working tree and original metadata remain in place. No
other checkout is accessed. The runtime handoff records the actual pushed SHA.
