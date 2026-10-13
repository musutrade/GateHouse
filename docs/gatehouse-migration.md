# GateHouse source migration

GateHouse is the development repository for the existing `harness-gate` CLI and
crate. The initial source snapshot retains the original MIT license and author
attribution. The original commit and repository are recorded in
[the migration binding](../.github/repository-migration.json). Earlier commits,
issues, immutable release tags, signatures and evidence remain in
[Harness-Gate](https://github.com/musutrade/Harness-Gate).

## Source and optional data

The import contains no old Git ancestry beyond GateHouse's initial README.
A normal complete clone therefore does not transfer old evidence blobs. Source,
test code, small compile-time fixtures, schemas, policy and current documentation
remain tracked. Exactly 1,480 large data/log/report files are served by the
[pinned data release](https://github.com/musutrade/Harness-Gate/releases/tag/source-data-20261013).
Their original paths/bytes are restored for full regression, including recorded
historical failures. Some frozen Markdown contains local data links; prepare the
data to read those links locally, or browse the original source revision in
Harness-Gate. [Preparation and offline use](../test-data/README.md).

## First-comparison continuity

Only the transition from seed `691447fbe5af64d01995d9e15c35781df4945e0d`
uses `migration_baseline.py`. It fetches the exact accepted legacy source commit
from the migration binding into CI only, verifies that the product source and
schemas were not changed by import, and supplies that SHA to collection and
projection. Ordinary later changes use their actual event base unchanged.
Each measured snapshot restores its own pinned regression inputs. No historical
debt, threshold, measurement series or required gate is reset or waived.
The legacy source commit is not a parent of GateHouse and is not included in
ordinary clones. A missing fetch or source mismatch blocks acceptance.

## Installation and release authority

Existing `--version v0.4.8` commands keep the original Harness-Gate release and
exact Sigstore workflow identity. Installer routing is explicitly versioned:
versions through v0.4.8 use Harness-Gate; v0.4.9 onward use GateHouse. There is no
fallback between signers or unsigned downloads. GateProbes plugin pins and
signing identities are unchanged. This migration does not publish a new binary
version or claim a new signing ceremony.

GateHouse keeps Required Quality Aggregate, mandatory PRs, strict base currency,
no force push/deletion, immutable `v*` tags, and the same core release reviewer
with administrator bypass disabled. Its release workflow rejects pre-cutover versions and verifies GateHouse's
own identity. The paused legacy bundled collector remains held and unprovisioned.
Old releases are neither copied nor re-signed.

## History and rollback

Use Harness-Gate for pre-migration blame and historical issues. Future issues and
PRs belong to GateHouse. Existing installations continue to use their immutable
version and can reinstall that version from the original owner. Source rollback
uses a reviewed GateHouse revert or the recorded legacy revision; no old Git
history is force-pushed into the lean repository.
