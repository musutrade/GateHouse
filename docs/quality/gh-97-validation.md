# GH-97 final verification and implementation handoff

Scope: OpenSpec tasks 9.1–9.4. This is an evidence handoff, with cross-platform
acceptance still open. No replacement baseline, threshold change, branch-protection
change or full-change acceptance is authorized by this record. See the
[acceptance index](../../openspec/changes/strict-json-results-and-risk-based-quality-gates/validation-gh-97.md)
and [ADR-0039](../adr/0039-required-risk-and-traceability-gates.md).

## Identity and local commands

The measured implementation is `381d5f418655969d29b81f89069845e6667f35c9`
(merged GH-96 / PR #107). This handoff adds documentation and evidence only.
Submitted-PR checks must identify their own tested SHA; these results are not
relabeled as measurements of a later documentation commit.

[Environment](https://github.com/musutrade/Harness-Gate/blob/d9a932b23dcfea709175285b594ca200450a9971/docs/quality/gh-97/environment.json): Linux x86_64, target
`x86_64-unknown-linux-gnu`, rustc 1.97.1 (`8bab26f4f`, LLVM 22.1.6),
Cargo 1.97.1 (`c980f4866`), nextest 0.9.143 (`60fa45f63`),
cargo-llvm-cov 0.9.0, Python 3.14.4, OpenSpec 1.10.0 and Node 24.18.0.
The machine-readable record retains full version output and UTC timestamps.

[Command ledger](https://github.com/musutrade/Harness-Gate/blob/d9a932b23dcfea709175285b594ca200450a9971/docs/quality/gh-97/commands.json) records exact commands, exits, elapsed times
and logs, including failed attempts. Complete logs are in
[local evidence](https://github.com/musutrade/Harness-Gate/blob/d9a932b23dcfea709175285b594ca200450a9971/docs/quality/gh-97/local-evidence.tar.gz).

| Command | Actual result |
| --- | --- |
| `cargo nextest run --manifest-path tools/harness-gate/Cargo.toml --locked` | Exit 0; 315 passed, zero skipped |
| `cargo fmt --manifest-path tools/harness-gate/Cargo.toml -- --check` | Exit 0 |
| `cargo clippy --manifest-path tools/harness-gate/Cargo.toml --all-targets -- -D warnings` | Exit 0 |
| `python3 -m unittest discover -s tools/quality/tests -v` | Exit 0; 119 tests passed |
| `python3 -m unittest discover -s tools/release/tests -v` | Exit 0; 23 tests passed |
| `python3 -m py_compile tools/quality/*.py tools/quality/tests/*.py tools/release/*.py tools/release/tests/*.py` | Exit 0 |
| `python3 tools/quality/contracts.py --output target/quality/gh97-local/contracts.json` | Exit 0 after target correction; 20/20 scenarios passed |
| `python3 tools/quality/contracts.py --output target/quality/gh97-local/contracts-structured.json --structured` | Exit 0 after target correction; 20/20 scenarios passed |
| `python3 tools/quality/docs_consistency.py --output target/quality/docs-consistency.json` | Exit 0; generated schema matches committed schema, examples and machine schemas pass |

Standalone Rust checks and the first docs check used
`CARGO_TARGET_DIR="$PWD/target/build"`. The contracts runner hardcodes
`tools/harness-gate/target/debug/harness-gate`; its first two attempts exited 1
with `FileNotFoundError` despite a successful build in the other target.
Unsetting the override also exited 1: Cargo tried the ambient shared
`/home/gem/cargo-target/debug/.cargo-build-lock`, which is read-only
(`os error 30`; nested Cargo exit 101). Both contract modes then passed with
`CARGO_TARGET_DIR="$PWD/tools/harness-gate/target"`. No product code or snapshot
was changed to accommodate the environment.

`harness-gate config check` and `harness-gate verify --profile ci --all` are
**not applicable and were not run against this checkout**: it has no
`.harness-gate/flow.toml` declaring a `ci` profile. Contract tests create their
own temporary project fixtures; those do not establish project-local applicability.

The passing quality suite includes CLI negatives for failed/cancelled/skipped
required stages and missing critical-path evidence, and stale identity/raw-file,
missing artifact, symbol, coverage and observable failures. The CLI negatives
assert nonzero exits. Contracts also check expected failure exits (1 or 2) against
actual exits, rather than requiring every scenario to exit zero. Individual
results are retained in the logs and [contract report](https://github.com/musutrade/Harness-Gate/blob/d9a932b23dcfea709175285b594ca200450a9971/docs/quality/gh-97/contracts.json).

## Extension gates and raw evidence

Reproduce at the measured implementation SHA with a fresh output directory:

```sh
python3 tools/quality/ci_quality.py collect \
  --base-sha 381d5f418655969d29b81f89069845e6667f35c9 \
  --head-sha 381d5f418655969d29b81f89069845e6667f35c9 \
  --run-id gh97-final-local --output target/quality/gh97-candidate
python3 tools/quality/ci_quality.py verify \
  --base-sha 381d5f418655969d29b81f89069845e6667f35c9 \
  --head-sha 381d5f418655969d29b81f89069845e6667f35c9 \
  --run-id gh97-final-local --output target/quality/gh97-candidate
```

The Git metadata is read-only. Collection uses a copy of this checkout's Git
metadata under ignored `target/gh97-git`, via a wrapper scoped to this workspace.
No other workspace or source checkout is accessed. Collection creates its own
build directories. Base and head intentionally identify the same implementation,
independently archived and measured. This checks final reproducibility; the
[GH-94 comparison](gh-94-validation.md) remains the before/after refactoring
evidence. It does not accept a baseline or establish absence of historical debt.

See [candidate manifest](https://github.com/musutrade/Harness-Gate/blob/d9a932b23dcfea709175285b594ca200450a9971/docs/quality/gh-97/candidate.json),
[raw candidate archive](https://github.com/musutrade/Harness-Gate/blob/d9a932b23dcfea709175285b594ca200450a9971/docs/quality/gh-97/candidate.tar.gz) and
[archive hashes](gh-97/SHA256SUMS). The archive retains the manifest and every
hashed artifact, including source snapshots, raw LLVM/LCOV/Cobertura, function
risk counters, isolated path runs and command logs. Build caches are excluded.
Collection timings are observed local wall times with other checks overlapping,
not isolated benchmarks or hosted-runner timings.

Collection exited 0 in **397.00 seconds**. Both the original directory and a
fresh extraction of the archive passed `ci_quality.py verify` (exit 0), checking
all **137 hashed artifacts**.

| Stage | Result | Local wall time |
| --- | --- | ---: |
| Original six-module coverage | All six and aggregate pass; 9,749/11,309 lines (86.21%) | 75.64 s |
| Extended production coverage | All ten and aggregate pass; 9,844/11,470 lines (85.82%) | 2.25 s |
| Independent base/head function risk | 212 identities; zero selected-function or comparison failures | 66.63 s |
| Isolated critical paths | 12/12 applicable rows pass (100%); all six mandatory paths pass | 252.12 s |

Production region coverage is separately 14,039/17,037 (82.40%); function coverage
is 830/1,114 (74.51%). The blocking production metric is line coverage. No branch
coverage is inferred from these other counters; unsupported branch measurement
and nonselected historical debt retain the existing policy treatment.

## Hosted evidence and platform applicability

[GitHub evidence](https://github.com/musutrade/Harness-Gate/blob/d9a932b23dcfea709175285b594ca200450a9971/docs/quality/gh-97/github-evidence.json) records all implementation issues
#88–#96 as closed/completed when retrieved. GH-96
[PR #107](https://github.com/musutrade/Harness-Gate/pull/107) merged;
its [PR CI run](https://github.com/musutrade/Harness-Gate/actions/runs/34112797811)
passed all required PR jobs. Its candidate measured the PR merge SHA
`ba99094fd6da0f9efe2429412a414ac9506b8485`, not the PR branch head
`d5efbb17c6a8f8005ea26317d2d455a6a773e3c5` or the squash commit.
The [hosted manifest](https://github.com/musutrade/Harness-Gate/blob/d9a932b23dcfea709175285b594ca200450a9971/docs/quality/gh-97/gh96-hosted-candidate.json) records all four stages
successful and 137 hashed artifacts, on Linux with Rust/Cargo 1.98.1 and
Python 3.14.7. Push-only platform jobs were skipped by PR policy; PR aggregate
success does not certify those jobs.

The retained [earlier push run](https://github.com/musutrade/Harness-Gate/actions/runs/34110236230)
measured **`c0e612351c2efd515fde96bcefad80c1365534a2`**, before GH-96.
Each platform's CLI contracts passed 20/20 scenarios:

| Evidence | Target | Command |
| --- | --- | --- |
| [Linux](https://github.com/musutrade/Harness-Gate/blob/d9a932b23dcfea709175285b594ca200450a9971/docs/quality/gh-97/linux-historical-contracts.json) | `x86_64-unknown-linux-gnu` | `python tools/quality/contracts.py --output target/quality/contracts.json` |
| [macOS](https://github.com/musutrade/Harness-Gate/blob/d9a932b23dcfea709175285b594ca200450a9971/docs/quality/gh-97/macos-historical-contracts.json) | `aarch64-apple-darwin` | Same command with `--structured` |
| [Windows](https://github.com/musutrade/Harness-Gate/blob/d9a932b23dcfea709175285b594ca200450a9971/docs/quality/gh-97/windows-historical-contracts.json) | `x86_64-pc-windows-msvc` | Same command with `--structured` |

All three reports record rustc 1.98.1 (`48a229cea`), Cargo 1.98.1 (`797e8a9bc`)
and Python 3.14.7, with exact OS and timestamps. Original downloaded artifact
IDs and SHA-256 hashes are in [download provenance](https://github.com/musutrade/Harness-Gate/blob/d9a932b23dcfea709175285b594ca200450a9971/docs/quality/gh-97/downloaded-artifacts.json).

That push run **failed overall**. macOS nextest failed
`extracted_cli_handlers_preserve_text_json_and_hook_snapshot_contracts` with
`HGCFG-INVALID-PATH` at `paths.audit_config`: the temporary project audit path
could not be resolved safely under `/var/folders/...`. The macOS benchmark also
failed because its cold nextest sample failed. The exact job logs are in the
local archive. These failures have not been repaired or revalidated by this
documentation-only issue. Linux and Windows test/build successes in that run
do not erase the macOS failure.

No final-SHA hosted macOS/Windows evidence is accepted here. The initial lookup
of push run `34113753638` at the measured SHA reported queued, with no conclusion;
that is a historical observation, not its current status. This handoff does not
poll or certify that run or the forthcoming PR checks.

The reviewed [path matrix](../../tools/quality/critical_paths.toml) and
[mandatory policy](../../tools/quality/critical_paths_policy.json) retain owner
`Harness-Gate maintainers` and review date `2026-09-07`:

| Host | Applicable isolated paths | Applicability review |
| --- | ---: | --- |
| Linux | 12 | Collected locally at the implementation SHA; six mandatory paths |
| macOS | 12 | No isolated collection accepted at the final SHA; six mandatory paths remain required |
| Windows | 7 | No isolated collection accepted at the final SHA; four mandatory paths remain required |

Windows excludes `process.timeout`, `process.cancellation`, `step.non_zero`,
`parser.failure` and `parser.json_false_positive` because those fixtures use
POSIX commands/signals. These are existing reviewed policy exclusions, not
missing-tool waivers. Structured CLI parser contracts still run on Windows;
they do not substitute for isolated source-linked matrix evidence.

Service ownership/cleanup fixtures use fake Docker/Podman executables and
observable lease/removal behavior. They establish deterministic core behavior,
not real-container success. `docker info` exited 0 locally (Docker 29.6.2);
daemon availability alone is not a container lifecycle test. No real-container
lifecycle is certified. The runtime adapter remains informational under the
existing coverage policy; it is not silently counted in core coverage.

## Acceptance and controller handoff

Task 9.3 remains unchecked pending evidence for the final implementation across
platforms and resolution/revalidation of the recorded macOS failure. Full-change
acceptance and replacement-baseline acceptance remain open. Strict OpenSpec
validation checks specification structure; it cannot waive these evidence gaps.
The PR transfers this record to the controller for required checks and delivery;
it does not declare a completed deployment or a green final platform matrix.
