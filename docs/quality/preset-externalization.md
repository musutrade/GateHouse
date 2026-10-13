# Ecosystem preset externalization and consumer review

Core embeds the generic execution preset and generic composition/security data.
GateProbes owns the three ecosystem flow templates, recipes and three packs.
`--presets-dir PATH` (or `HARNESS_GATE_PRESETS_DIR`) selects a plain local catalog;
there is no package discovery, download, or embedded ecosystem fallback.
[Usage and catalog contract](../quality-presets.md) document the boundary.

## Preserved inputs and outputs

The nine original files come from Core
`da9a1082f698f17418b9d20ed077bd1aca2adfc8`. GateProbes records each source path
and SHA-256 in `docs/preset-migration.json`. Core retains the same bytes only in
`tools/harness-gate/fixtures/preset-compat`, with an exact inventory and digests
in `provenance.json`. Its catalog is a test adapter. `test_preset_packs` rejects
changed or added frozen files; product code never selects this fixture directory.

GateProbes `tools/presets/accept.py` compares generated flow, quality fields,
policy/series/capability files and full/CI collector/policy participation for
all three presets. Its required CI job runs against digest-pinned released Core
0.4.8 to establish migration parity. The same checker with `--external` passed
against the candidate Core binary and the actual GateProbes preset directory.
The released binary predates the loader; these are two distinct acceptance paths.

Core tests cover unknown ecosystem names through the same loader, flag/environment
precedence, absent catalogs, duplicate/invalid JSON, unsupported catalog versions,
reserved names, escaping and symbolic-link paths, oversized/non-file inputs,
output conflicts and preserved reference semantics. Existing configuration
validation, reference-equivalence and ordinary CI checks remain required.

## Source and fixture consumer review

| Material | Actual retained consumers | Decision |
| --- | --- | --- |
| Former ecosystem files under `tools/harness-gate/presets` | Init previously embedded them | Remove from production templates; GateProbes owns current versions |
| Frozen preset copies | Rust config/collector/scope/preset tests; Python preset/arc-admin tests; native policy/config test helpers; workflow compiler acceptance; executable documentation checks | Keep as explicit test fixtures with byte provenance |
| `rust-reference` and historical quality documents/archives | `test_rust_reference`, generic quality replay and historical equivalence | Keep Core-owned; no regenerated history or new measurement identity |
| Native Python helpers, driver/plugin/runtime source and `rust-native` fixtures | Delegated test digest guards; retained runtime/delivery/contract tests; reviewed legacy reproduction | Keep compatibility source; native product maintenance and release belong to GateProbes |
| `rust-measure` source | Retained experimental `test_rust_native` builds its historical analyzer on its supported compiler | Keep bounded legacy consumer; ordinary Core measurement uses the existing immutable release pin |
| `rust-source-risk` source/tests | Exact-tree handoff guard and rollback of the transferred standalone step | Keep frozen rollback inputs; no ordinary Core build/test step |
| TypeScript historical adapters and `typescript-angular` fixtures | Acceptance, advisory, contracts, semantics, reference and generic policy tests | Keep: the new original-source plugin is not an equivalent Angular/V8 historical adapter |
| Migrated TypeScript/HTTP plugin trees | Reviewed source/release rollback and the explicit Core-only isolation exclusion inventory | Retain compatibility copies; independent GateProbes releases already own the active product |

The review does not infer deletion safety from a file's language or directory.
Core CI still runs all compatibility/reference modules without demonstrated
replacement parity. The [test handoff](ci-plugin-test-handoff.md) records the
bounded execution transfer. No archived evidence, immutable release, old signature,
measurer pin, or paused bundled-runtime release chain is changed.

## Rollback

Reverting this migration restores the previous embedded catalog and file paths.
Historical files remain byte-identical and the exact source commit is recorded.
Existing initialized project files need no regeneration. GateProbes preset changes
can proceed independently once a user explicitly selects that reviewed directory.

## Isolated acceptance

The [machine-readable result](preset-isolation-acceptance.json) records Core
`d1a8bb406ac16594ac3707cfa794c99744aac001` with all 24 declared plugin
implementation paths removed: build passed, 497 Rust tests passed, and 441
selected Core Python tests passed without skips. This is the Core-only isolation
selection, not a claim that historical compatibility tests can be deleted.
Subsequent changes renew the ordinary CI receipt, classify the existing
`#[cfg(test)]` scope tests correctly, and add catalog boundary cases; they do not
change the tested production loader. GateProbes PR #15 merged as
`ae7a3a200817c67081d52a58cd580b1be12631c4` after ordinary CI and all four native
release-acceptance platform jobs passed.


## Issue #10 ordinary CI handoff

The [accepted v2 handoff](ci-plugin-test-handoff.md) now preserves the remaining
historical adapter tests in GateProbes and proves the fixed Core policy/preset
inputs against their original producer. Ordinary `Quality Script Tests` removes
all 24 plugin implementation paths from its disposable checkout and runs the
same guarded Core selector, including generic TypeScript policy and preset tests.
This supersedes the manual 15-module exclusion as current acceptance; historical
isolation results above remain historical evidence. Core decisions, measurement
series, thresholds, baseline/debt/ratchet and release authority are unchanged.
