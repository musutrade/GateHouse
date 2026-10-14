# Core/plugin split reconciliation — Issue 15

Reconciled on 2026-10-14 against GateHouse main
`93a330186c9b80a95c5b2c89244d03105cdbeb89` and GateProbes main
`3a58d3df302f6e08857f4ce829e073ecc97d087a`, supplemented by the native
protection fix merged as `9571cffe6cb6641012dab0353cc704910d9f61ed`.
Engineering Policy semantics are unchanged: collectors measure; Rust Core retains
final decisions, requiredness, thresholds, measurement series, baseline/debt/ratchet
and release authority. No baseline is adopted or historical evidence rewritten.

## Delivered scope

| Dimension / issue | Accepted result | Evidence and limits |
| --- | --- | --- |
| Ordinary Core CI / GateHouse #10 | Closed; retained Core checks run without plugin implementation source | [Handoff](../ci-plugin-test-handoff.md), GateProbes [CI 38406597322](https://github.com/musutrade/GateProbes/actions/runs/38406597322): 103 inherited methods, no failures/errors/skips; eleven paused bundled-runtime methods recorded as held |
| Measurer consumer pin / GateHouse #11 | Closed; exact GateProbes 0.3.1 identity replaces the old artifact declaration after continuity verification | [Consumer acceptance](../issue-11/README.md): identical executable/capability bytes and 201 output comparisons; explicit legacy rollback pin; [merged-main CI](https://github.com/musutrade/GateHouse/actions/runs/38495521539) passed |
| Paused legacy publication / GateHouse #12 | Closed by retirement, with bytes and publication hold preserved | [Retirement](../legacy-collector-retirement.md); any revival belongs to GateProbes and requires a separate product decision and reviewed migration |
| Physical Core source cleanup / GateHouse #13 | Closed; 24 implementation roots and 13 delegated test copies removed | [132-file inventory and rollback](../issue-13/README.md); 39 legacy files frozen for 64 original Core protection tests; [merged-main CI](https://github.com/musutrade/GateHouse/actions/runs/38501723197) passed |
| Independent release delivery / GateHouse #14 | Closed; Core 0.4.9 GitHub and crates.io delivery completed | [Original signed public receipt](../../releases/0.4.9-public-verification.md), [crate archive/source and fresh install](../../releases/0.4.9-crate-public-verification.md); PR #21 merged future credential preflight; original failure retained |
| Current Core/plugin compatibility / GateProbes #16 | Closed; all five current plugins accepted against downloaded Core 0.4.9 | [Accepted run](https://github.com/musutrade/GateProbes/actions/runs/38406597322) and [scope record](../ci-plugin-test-handoff.md): native Linux/Windows/macOS Intel/macOS ARM; four other plugins Linux; native classification remains Linux-only |
| Native RC asset protection / GateProbes #17 | Closed by documented legacy limitation and mandatory future protection; RC8 remains published, prerelease and mutable | [Merged safeguards PR #19](https://github.com/musutrade/GateProbes/pull/19), [machine receipt](https://github.com/musutrade/GateProbes/blob/9571cffe6cb6641012dab0353cc704910d9f61ed/docs/quality/issue-17/rc8-public-verification.json); tag update/deletion rules do not lock existing RC8 assets |

The original four-package [publication acceptance](../independent-plugin-release-acceptance.md)
is a dated Core 0.4.8 receipt. The later Core 0.4.9 matrix supplements it; neither
receipt is rewritten. Core 0.4.9's four binaries passed release builds; fresh Core
installer and crates.io runtime verification are Linux x86_64. That scope is
separate from the native plugin's four-platform hosted acceptance.

OpenSpec 6.4 now has its required actual GateHouse signing/provenance and install
receipt. This does not declare every ecosystem stable, every platform supported,
or the paused bundled product resumed. The native RC remains RC8; four other
independent plugin releases have their own immutable asset protection.

## Retained ownership and rollback boundaries

| Retained material / owner | Reason and consumers | Rollback boundary |
| --- | --- | --- |
| Core/shared governance / GateHouse | Host/evidence schemas, generic policy/reference fixtures, shared release policy/inventory/SBOM sources | [Schema](../schema-ownership.md), [fixture](../fixture-ownership.md), [release ownership](../release-ownership.md); generated plugin copies remain pinned |
| Frozen historical plugin input / GateHouse tests, GateProbes product owner | Actual policy/reference consumers and original handoff replay; frozen legacy rejection/lifecycle tests | [Cleanup inventory](../issue-13/README.md) fixes original hashes and restore commit; revert selector/ownership/removal together |
| Archived bundled release / GateProbes revival owner | Paused product, restored only for a separately reviewed decision | [Retirement](../legacy-collector-retirement.md); restoring bytes keeps the hold and does not authorize publication |
| Native RC8 assets / GateProbes release owner | Existing 30-file, ten-subject release; immutable setting applies to future releases only | Keep exact tag/source/assets/signatures; verify pinned digests and exact identity; never delete/recreate, replace or resign RC8 |

[GitHub's documented setting](https://docs.github.com/en/code-security/how-tos/secure-your-supply-chain/establish-provenance-and-integrity/prevent-release-changes)
only applies to future releases. GateProbes currently has it enabled. [GateProbes PR #19](https://github.com/musutrade/GateProbes/pull/19) supplies fresh
30-file/ten-subject verification and merged future safeguards. The publication
job now verifies actual postpublication `draft`, `prerelease`, `immutable`,
exact asset bytes, signing identity and source, and retains a machine receipt.
The settings API requires Administration:read; settings inspection uses the
operator's existing authentication, while ordinary publication verification uses
its normal Actions token without a new administrative credential. Legacy RC8 must be reported as `legacy-mutable`,
never as fully protected. Its existing tag rules prohibit update/deletion without
bypass, but signatures/digest verification remain necessary for asset consumption.

## Closure condition

All seven child issues are closed. GateProbes #17 closed when its safeguards
merged after the exact candidate `dd1d8143814bc419f87a019a8fbfea6271a77989`
passed [ordinary CI 38511920703](https://github.com/musutrade/GateProbes/actions/runs/38511920703)
and [four-platform native CI 38511920712](https://github.com/musutrade/GateProbes/actions/runs/38511920712).
The historical machine receipt records `integrity_status=pass` and
`protection_status=legacy-mutable`; normal verification rejects RC8's mutable
state. No tag-triggered publication was performed for this fix. This record
completes Issue #15's remaining OpenSpec, ownership, retained-material and scope
reconciliation; closing the umbrella does not claim RC8 was retroactively locked.
Full plugin matrices are not rerun inside ordinary Core CI; dated accepted hosted
runs and bounded compatibility receipts retain the existing cost model.
