# Core CI plugin test handoff

This change transfers only tests whose execution is already demonstrated in
GateProbes. Engineering Policy semantics, required check names, Core platform
coverage, measurement identities, thresholds and release authority are unchanged.

## Accepted handoff

The [receipt](ci-plugin-test-handoff.json) records the exact GateProbes commit,
successful CI run, test paths, execution commands and retained source digests.

| Core execution | GateProbes execution | Evidence |
| --- | --- | --- |
| `test_rust_collector_project` | Native plugin helper tests through `tests/zero_skip.py` | Same eight test methods and bytes; any skip fails |
| `test_rust_native_driver` | Native `accept.py` loads `NativeDriverTests` with the real driver, sysroot and pinned Core | Same ten test methods and bytes; four platforms; any skip fails |
| `test_native_artifact_paths` | Native `accept.py` on four platforms | Three identical test-method ASTs; canonical temporary-root setup adaptation |
| `test_rust_native_classify` | Native `accept.py` on Linux | Eight identical test-method ASTs and fixture bytes; real driver, packaged wrapper, any skip fails |
| Standalone `rust-source-risk` AST build/tests and Python discovery | Registry-selected Python plugin job builds its inventory, runs Cargo tests and Python discovery with `RUST_SOURCE_INVENTORY` | Same six Python test files and both AST Rust source files; no platform coverage removed |

The source-risk discovery includes the same static toolchain compatibility
contract as before. The separately invoked expensive dual-toolchain acceptance
was not part of either discovery command and is not newly claimed here.

`Quality Script Tests` uses
[`run_core.py`](../../tools/quality/tests/run_core.py). It first checks the
handoff and retained source digests, then runs every discovered `test_*.py`
module except the four accepted native modules. New modules run by default.
Missing or modified delegated tests or guarded plugin/Core inputs fail selection;
restore local execution or renew the reviewed parity evidence before changing
those retained implementations. The check uses committed evidence, without a
GateProbes checkout or a new network dependency in Core CI.

The eight project tests also depend on Core's `harness_evidence.py`,
`project_model.py`, `quality_evidence.py` and `collector_runner.py`, plus
`harness-evidence.schema.json` and `project-model.schema.json`. All six inputs
are digest-guarded alongside the existing plugin binding schema. Their bytes
match Core `8d93ea53c7cf8b314bda816eafe13209da25c742`, which the accepted
GateProbes CI pins through the top-level `source_commit`. Changing a Core
consumer or either schema invalidates that handoff even if plugin code is
unchanged. Renew acceptance against the changed inputs or restore the eight
local project tests under a revised handoff; updating hashes alone is not
renewed acceptance.

The original test files remain available: legacy runtime tests still import
fixture helpers from `test_rust_collector_project`. This change transfers
execution and does not delete those dependencies.

## Retained coverage

The fifteen exclusions in `core_isolation_check.py` serve a different purpose:
they define the manually tested Core-only snapshot. They are not evidence that
all fifteen modules have been accepted by GateProbes. Ordinary CI retains the
other eleven modules, with their individual reasons recorded in the receipt:

- Core preset and generic policy/reference semantics.
- Historical TypeScript/Angular/V8 adapters, archives, normalization and advisory
  behavior. The new TypeScript plugin does not support the same mapped reports.
- Legacy native delivery and bundled-runtime contracts without complete
  replacement coverage in GateProbes.

Core reference equivalence and its `rust-reference` fixture directory remain
Core-owned. Release governance and retained compatibility tests remain in Core.
The later
[four-plugin publication handoff](independent-plugin-release-acceptance.md) removes
plugin packaging from Core release after signed public installation acceptance.
That separate handoff does not complete ordinary Core CI independence or remove
the retained source/fixture consumers described here.

## Verification and rollback

Focused tests cover stale source/test digests, changes to the project tests'
Core consumers and evidence/model schemas using the actual receipt, missing
inputs, malformed or unpinned handoffs, missing execution evidence and
conservative selection of new tests. Existing CI topology tests continue to
enforce the unchanged required job and platform contract. The fixture ownership test guards the historical
Core Python equivalence inputs as well as Rust fixture references.

Reverting the workflow change restores the original discovery command and
standalone source-risk execution. No historical release, signature or baseline
is changed. Hosted checks on the PR supply the required acceptance; no numerical
CI speed improvement is claimed without a measured comparison.

## Remaining ordinary CI handoff (2026-10-13)

GateProbes [PR #15](https://github.com/musutrade/GateProbes/pull/15) and
[successful CI run 38342999155](https://github.com/musutrade/GateProbes/actions/runs/38342999155)
accept the additional 3 path cases on four platforms and 8 classification cases
on Linux. Core's original Quality Script Tests job is Linux-only; no original
platform coverage is removed. GateProbes acceptance runs 34 cases on Linux and
26 on other hosts, plus 8 helper cases per host, with zero skips.

An attempted Windows classification run failed closed on an unowned compiler
instance. That historical fixture has no accepted Windows/macOS contract; the
limitation is recorded in GateProbes rather than changing measurement semantics
or claiming additional platform parity. All four original platform suites remain
required. Source digests also guard all 14 newly transferred fixture files.

The native policy helper relocation changes one Core template path only. The
old and new template bytes match the recorded source commit; a frozen inventory
test guards their continued identity. The helper's existing GateProbes setup
adaptation and unchanged policy behavior are recorded separately in the receipt.
See [preset externalization and consumer review](preset-externalization.md) for
why compatibility sources and historical materials remain in Core.
