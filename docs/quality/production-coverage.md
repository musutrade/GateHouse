# Production coverage boundaries (candidate)

GH-91 implements OpenSpec tasks 3.1–3.5. The versioned
[production-source inventory](../../tools/quality/production-source.json)
assigns every Rust file under `tools/harness-gate/src` exactly once to a boundary
or a reasoned exclusion. New, missing or multiply assigned sources fail closed.
The ten blocking boundaries each require at least 80% executable line coverage;
their aggregate sums raw counts and independently requires 80%. Other production
modules remain explicitly informational and cannot improve that aggregate.

This is a new candidate series, `production-location-1`, under the
[measurement contract](measurement-contract.md) and
[OpenSpec design](../../openspec/changes/strict-json-results-and-risk-based-quality-gates/design.md).
It does not replace the historical measurements in
[ADR-0025](../adr/0025-phase-1-quality-baseline-gates.md).
GH-96 task 8.1 adds the production gate alongside the preserved six-module gate
on PRs and pushes. The historical GH-91 candidate below failed the threshold;
no baseline acceptance is claimed. Current adoption and measurement limits are
recorded in [ADR-0039](../adr/0039-required-risk-and-traceability-gates.md).

## Service mapping

| Sources under `src/service/` | Boundary and reason |
| --- | --- |
| `commands.rs`, `inspection.rs` | Core: deterministic command building and response parsing moved out of the runtime adapter, with their existing tests. |
| `lease.rs` | Core: immutable identity, ownership checks, lease/heartbeat state, and cleanup decisions. Fake runtime tests exercise these actual implementations. |
| `mod.rs`, `docker.rs`, `postgres.rs` | Core: service orchestration, validation, environment and log decisions. Mixed orchestration remains conservatively blocking, including `docker.rs`; its name does not make it an adapter exclusion. |
| `runtime.rs` | Informational adapter: Docker/Podman command execution and runtime selection behind `ContainerRuntime`. |
| `tests.rs` and inline `cfg(test)` items | Test exclusions recorded as files or source ranges with reasons. |

The extraction preserves command arguments and inspection behavior. No daemon
availability is inferred from fake runtime tests or local coverage.

## Counting and exclusions

`production-source-1` accepts only test, generated, or benchmark-only exclusions
with reasons. The current inventory has no generated source exclusions.
Integration test sources are separately identified under `tests/`. The lexer
excludes complete `#[cfg(test)]` items (including helpers) and `#[test]` functions,
while ignoring delimiters inside comments and Rust string/character literals.
It retains `cfg(not(test))`, handles `cfg(all(test, ...))`, and rejects unsupported
test predicates and mixed production/test source ranges instead of guessing.

Each executable physical line is counted once. JSON segments reconstruct
[LLVM LineCoverageStats](https://github.com/llvm/llvm-project/blob/release/20.x/llvm/lib/ProfileData/Coverage/CoverageMapping.cpp)
and must agree exactly with LCOV `DA` records. Native LLVM summary line counts
can include overlapping instantiation mappings and are not this denominator.
Source function and code-region locations are deduplicated across instantiations,
with a hit in any instance covering the location. Unhit production functions and
regions remain in their denominators. Mapping counts must match native per-file
function/region totals before test exclusions; missing mappings fail closed.
Line, function and region counts remain separate; only lines gate these boundaries.

Ten declaration-only files have no LLVM executable mapping. They remain assigned
in the inventory and contribute zero counts, with individually reviewed reasons
and pinned source hashes. A changed hash requires review; other missing file
reports fail. JSON, LCOV and inventory digests, source digests, per-file counters,
and excluded ranges are retained in the generated report. These controls reject
empty reports/boundaries, unknown paths, duplicate assignments or reports, and
missing or mismatched evidence. Threshold comparisons use unrounded raw counts.

## Reproduction and observed evidence

Run from this checkout (the environment's default Cargo target is read-only):

```bash
export CARGO_TARGET_DIR="$PWD/target/rust"
cargo nextest run --manifest-path tools/harness-gate/Cargo.toml --locked
cargo fmt --manifest-path tools/harness-gate/Cargo.toml -- --check
cargo clippy --manifest-path tools/harness-gate/Cargo.toml --all-targets -- -D warnings
python3 -m unittest discover -s tools/quality/tests -v
python3 tools/quality/docs_consistency.py --output target/quality/docs-consistency.json
openspec validate strict-json-results-and-risk-based-quality-gates --strict
python3 tools/quality/coverage.py --production --output target/quality/production-coverage.json
```

The collector generates `coverage.raw.json`, `coverage.lcov`, and
`coverage.cobertura.xml` alongside `production-coverage.json` and its Markdown
summary. The same strict evaluation can be reproduced without recollection:

```bash
python3 tools/quality/coverage.py --production \
  --raw target/quality/coverage.raw.json --lcov target/quality/coverage.lcov \
  --output target/quality/production-coverage.json
```

Local Linux validation: 301 nextest tests passed, 0 skipped; fmt and Clippy passed;
52 quality unit tests passed, including 13 production-boundary tests; docs
consistency and strict OpenSpec validation passed. Negative fixtures cover empty,
unknown, duplicate, missing-report, missing-function/region, test-only inflation,
and below/exactly/above 80% cases. Existing service tests were moved, not added to
increase coverage.

The initial unmodified-environment nextest command failed with
`failed to open: /home/gem/cargo-target/debug/.cargo-build-lock` and
`Read-only file system (os error 30)` (Cargo child exit 101). Setting the writable
workspace target above resolved it. The unmodified-environment docs command also
returned 1 (`documentation, examples, or schema synchronization failed`): its
Cargo subprocesses need the same target override; the documented rerun passed.
An initial Clippy run caught an unused moved
test import; removal and rerun passed. `harness-gate config check` and
`harness-gate verify --profile ci --all` are **not applicable**, because this source
checkout has no `.harness-gate/flow.toml` or declared project-local `ci` profile.
No project configuration was invented. Full local logs are in
`target/quality/gh-91/` (ignored artifacts).

Publishing note: local `git add` fails because `.git/index.lock` cannot be created
on the read-only `.git` mount. The validated file contents are therefore committed
and published using GitHub's Git API on `symphony/GH-91`. Local Git metadata stays
at the base commit; the PR and runtime handoff identify the published commit.

The final candidate collection completed, and the coverage command returned **1**
for genuine coverage debt: app, doctor, project, service-core, verify, and the
aggregate are below 80%. This is successful fail-closed evidence generation,
not a passed production coverage gate. The blocking aggregate is
8,856/11,273 lines (78.56%). The following are covered/total raw counters:

| Boundary | Lines | Functions | Regions | Status |
| --- | ---: | ---: | ---: | --- |
| app | 201/364 | 7/17 | 277/579 | fail |
| audit | 806/975 | 97/120 | 1305/1630 | pass |
| config | 2327/2655 | 175/195 | 3149/3670 | pass |
| doctor | 44/227 | 6/28 | 69/382 | fail |
| process | 1063/1311 | 80/140 | 1469/1854 | pass |
| project | 244/422 | 31/57 | 364/692 | fail |
| scope | 97/113 | 6/8 | 134/157 | pass |
| secrets | 391/433 | 47/54 | 582/692 | pass |
| service-adapters | 0/110 | 0/13 | 0/143 | informational |
| service-core | 980/1367 | 83/147 | 1391/2093 | fail |
| verify | 2703/3406 | 213/334 | 3907/5118 | fail |
| aggregate | 8856/11273 | 745/1100 | 12647/16867 | fail |

Collection identified base HEAD `6ffdf139512a09a1fc50e088af89b8cfbd153be5` with
uncommitted GH-91 source changes; source SHA-256 values in the generated report
identify the measured files. Collection used Linux x86_64, Cargo dev,
Rust 1.97.1, cargo-nextest 0.9.143, and cargo-llvm-cov 0.9.0.
Raw input hashes for this observation:

- `production-source.json`: `8e0000bd038875e15cbc4ef5d957c789d63dd5c1590e25757a650a3b9aec89ac`
- `coverage.lcov`: `c30511dcfa03b3b40cbcc338ed76043f86d01fb8c509776c5b3849ea1c5d78e5`
- `coverage.raw.json`: `2958661e998d6519b849da20014e1874f487b26c508990ea7eac2501623907ae`
