# Plugin source cleanup after accepted consumer transfer

Engineering Policy semantics are unchanged. Collectors measure facts; Rust Core
retains final decisions, requiredness, thresholds, measurement series,
baseline/debt/ratchet and release authority. No waiver or exclusion is widened,
and no baseline, historical acceptance result or signature is rewritten.

## Removal and ownership

[The per-file inventory](removal-inventory.json) records 132 former active files,
their original SHA-256, owner, prior consumer, frozen destination where needed,
and byte-identical GateProbes paths found in the accepted revision. All 24
implementation roots from the original isolation list are absent, together with
13 delegated Core test copies. The isolation list and handoff digests stay exact.

GateProbes revision `a762f0c4c7da3945367b870bccc39aac8069132f` and accepted
[run 38406597322](https://github.com/musutrade/GateProbes/actions/runs/38406597322)
carry the continuing native, TypeScript, HTTP, source-risk and measurer products
and historical compatibility scope. The original
[handoff](../ci-plugin-test-handoff.md), [independent release acceptance](../independent-plugin-release-acceptance.md)
and [measurer review](../issue-11/README.md) provide execution/signature evidence.
The inventory records Git blob matches against that exact GateProbes tree;
an empty match list is explicitly not a byte-parity claim. Native packaging/path
adaptations use the existing native handoff, while unused documentation and
old runtime/manual entries are recoverable from the exact Core restore commit.
Runtime methods held by the accepted receipt remain held, not counted as passed.
Lockfiles and licensing assets travel with the independently released products;
Core's root license and shared SBOM machinery remain in Core.

| Material | Owner / actual consumer | Disposition |
| --- | --- | --- |
| 24 implementation roots and 13 delegated test copies | GateProbes accepted native/product and legacy compatibility jobs | Delete local copies; Core uses fixed artifacts and the accepted receipt |
| Legacy delivery scripts and six release-test modules | GateProbes revival owner; Core synthetic rejection/lifecycle protection suite | Preserve 39 original files under `retired/legacy-collector/snapshot`; run 64 original methods |
| Native and Angular fixture trees and guarded helper inputs | Frozen historical handoff inputs and retained Core policy/reference consumers | Retain; these are not active product implementations |
| Core protocol/schema, invalid-evidence, timeout/replay, rust-reference, generic policy and build fixtures | Core validation, full Rust and ordinary Core Python suites | Retain unchanged |
| `release_policy.py`, `release_inventory.py`, `generate-sbom.py` | Core/shared release governance | One editable Core source; the frozen tests link it in their temporary layout |
| Historical workflows, signed releases and raw evidence | Core historical verification and publication-hold tests | Retain unchanged |

The retained [legacy snapshot](../../../retired/legacy-collector/README.md) is
explicitly frozen and separated from active tools. This cleanup does not claim
that every historical plugin-related file was deleted. The former independent
packager is frozen because its historical identity and malicious-archive tests
remain consumers; active GateProbes packaging belongs to its own release chain.

## Enforcement and verification

`Quality Script Tests` now checks physical absence in the real checkout before
running its unchanged guarded selector. It no longer removes source at CI time.
Core release CI still discovers its normal suite, including the wrapper for the
frozen original tests. The wrapper verifies the exact method inventory; missing
files cannot become skips. Original handoff, producer parity, Core policy inputs,
schemas and accepted measurer series/digests are not refreshed by this cleanup.

Local verification is preserved in `validation.json`. Its original pending
hosted status records the pre-merge observation. Subsequent merged revision
`eb91e9fa28510994ffabceb06c4c7a2119a35fff` passed
[main CI 38501723197](https://github.com/musutrade/GateHouse/actions/runs/38501723197),
including the unchanged required aggregate; PR #19 merged and Issue #13 closed. The local release baseline is 102 passing methods; after migration,
all original 102 methods must still execute (38 active plus 64 frozen), with new
integrity regressions counted separately. No complete plugin matrix is added to
Core; the frozen Linux test scope preserves its original synthetic coverage.

## Rollback

Recover any listed file from GateHouse commit
`da8e8a58a4ac5959dc0346b79bad4a4437b964cb` at its original inventory path, and
check the inventory SHA-256. Revert the removal, ownership, cleanup assertion and
frozen runner together in a reviewed change. The accepted handoff receipts were
not changed, so restoring the original bytes preserves their digest checks.
Keep publication holds, repository/ref restrictions and approval protections.
Recovery does not authorize signing, publication, measurement migration or
baseline adoption. GateProbes owns any future revival of the bundled product.
