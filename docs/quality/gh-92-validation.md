# GH-92 implementation evidence

Scope: OpenSpec tasks 4.1–4.5 only, on `symphony/GH-92`. Dependencies
[#90](https://github.com/musutrade/Harness-Gate/issues/90) and
[#91](https://github.com/musutrade/Harness-Gate/issues/91) were closed before
implementation. The [risk contract](function-risk.md) and
[ADR-0025](../adr/0025-phase-1-quality-baseline-gates.md) describe the result.
This record validates tooling, not a production CRAP baseline or acceptance of
the remaining OpenSpec tasks.

## Reproduction and fixtures

The pre-change Python suite passed 52 tests. The final suite passes 85 tests,
including fixed analyzer records, a
[real LLVM export](../../tools/quality/fixtures/risk/README.md), and complete
synthetic retained bundles. The real export reproduced two generic and two
closure instances, one main function and a zero-hit function. It exposed LLVM
22's disjoint signature/body regions and required the analyzer's public-function
prefix spans and zero-argument closure fix. The analyzer version is now 0.1.1;
historical 0.1.0 evidence remains a distinct series.

| Task | Validated evidence |
| --- | --- |
| 4.1 | Same-name identities, generic deduplication, closure isolation, UTF-8 columns, unhit functions, gaps, unmapped/nested functions, source digest/commit mismatch, relocation without raw-export rewriting |
| 4.2 | Complete bundle produces `risk.json` and `risk.md`; 10.8 fixture; exact rational comparison immediately above 30; per-row raw-artifact links |
| 4.3 | Independent line/function/region counters; unsupported branch has reason/tool version and rejects counters; production boundary failure and high-risk line/region failures remain blocking |
| 4.4 | New/modified/moved CRAP, one-to-one identities, splits, unchanged line shifts, separate debt, selected hotspot move protection, retained passing boundary-test IDs |
| 4.5 | Missing base/fields, incompatible series/tools, stale timestamps/run IDs, corrupt/escaped artifacts, source mismatch, missing/uninstrumented CLI, missing profiles, skipped/failed tests, invalid/expired exceptions; valid exceptions do not waive failure |

The complete-bundle fixtures intentionally synthesize run provenance and mock
Git-source reads. Their success is consumer validation, not production test-run
attestation. The frozen LLVM export is independently generated compiler evidence.

## Local validation

Logs are retained locally under `target/quality/gh-92/` (ignored build artifacts).

| Command | Result |
| --- | --- |
| `CARGO_TARGET_DIR="$PWD/target/rust-validation" cargo nextest run --manifest-path tools/harness-gate/Cargo.toml --locked` | PASS: 301 tests, 0 skipped; `nextest.log` |
| `cargo fmt --manifest-path tools/harness-gate/Cargo.toml -- --check` | PASS; `fmt.log` |
| `CARGO_TARGET_DIR="$PWD/target/rust-validation" cargo clippy --manifest-path tools/harness-gate/Cargo.toml --all-targets -- -D warnings` | PASS; `clippy.log` |
| `python3 -m unittest discover -s tools/quality/tests -v` | PASS: 85 tests; `python-final.log` |
| `CARGO_TARGET_DIR="$PWD/target/rust-validation" python3 tools/quality/docs_consistency.py --output target/quality/docs-consistency.json` | PASS: links, examples, migration and schemas; `docs-consistency-final.log` |
| `openspec validate strict-json-results-and-risk-based-quality-gates --strict` | PASS: change valid; `openspec.log` |
| `harness-gate config check` | NOT APPLICABLE: no project-local `.harness-gate/flow.toml` |
| `harness-gate verify --profile ci --all` | NOT APPLICABLE: no project-local `.harness-gate/flow.toml` declaring `ci` |

The initially requested unmodified nextest command failed before compilation:
`failed to open /home/gem/cargo-target/debug/.cargo-build-lock: Read-only file
system (os error 30)`, from nextest's Cargo metadata build (exit 101). The
environment points its default Cargo target directory outside the writable
workspace. Overriding only `CARGO_TARGET_DIR` to the workspace allowed the full
test suite and clippy to complete. The first docs-consistency invocation also
failed its Cargo-backed checks with that default directory, and reported links
to this then-unwritten evidence record. Its final run uses the same writable
target directory. No generic project config was invented.

Toolchain: `rustc 1.97.1 (8bab26f4f 2026-07-14)`, Python 3.14.4,
LLVM `22.1.6-rust-1.97.1-stable`. Rust source is unchanged by this issue.

## Remaining acceptance

The current lexical analyzer and location mapper reject unsupported syntax or
expansions rather than accepting incomplete measurements. No whole-project CRAP
number is claimed here. Collection/CI integration, production hotspot evidence,
mandatory-path discovery and a separately reviewed candidate baseline remain
with the other OpenSpec tasks. Required PR checks and final delivery are owned
by the Symphony controller.

The workspace also mounts `.git` read-only: staging failed with
`fatal: Unable to create '/home/gem/symphony-workspaces/GH-92/.git/index.lock':
Read-only file system`. Delivery therefore publishes these validated file bytes
through the GitHub Git Data API on the same `symphony/GH-92` branch. Local Git
metadata stays at the prepared baseline; the handoff records the actual remote
commit. No other checkout or workspace is used.
