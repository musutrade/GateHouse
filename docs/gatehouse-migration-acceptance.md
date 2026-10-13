# GateHouse migration acceptance — 2026-10-13

Source externalization: [Harness-Gate PR #357](https://github.com/musutrade/Harness-Gate/pull/357),
merged `e8532697d67b6145ed22db471870fe0e3538b709`.
Lean import: [GateHouse PR #1](https://github.com/musutrade/GateHouse/pull/1),
merged `7f1fdac4b74b6bbf2ccb20cd4499d4ab4d497af5`.

## Actual complete clones

Both commands were ordinary `git clone --progress` over HTTPS, without depth,
filters, sparse checkout, local object sharing or an evidence download. These
are single observations on the same machine, not a network-speed guarantee.

| Measurement | Harness-Gate | GateHouse import |
| --- | ---: | ---: |
| Received Git object pack bytes | 40,155,724 | 2,806,866 |
| Object pack MiB | 38.30 | 2.68 |
| Elapsed seconds | 12.57 | 4.36 |
| Allocated checkout bytes (including .git) | 109,391,872 | 17,932,288 |

Object pack reduction: **93.01%**.
Pack bytes measure the received Git object payload; transport framing is not
included. The GateHouse import has 1,411 tracked files totaling
9,570,420 bytes (9.13 MiB). Its three reachable commits
are the seed, reviewed import branch and squash merge; the old source commit is
absent from the clone's object database. No old Git ancestry was imported.

## Build, regression and data identity

- All 1,480 external paths were absent before **and after** a clean ordinary
  `cargo build --locked`; the build completed successfully in an empty target
  directory without preparing any evidence data.
- From that fresh clone, all three public Release archives were fetched into
  an empty data cache. All 1,480 member sizes/hashes and archive sizes/hashes
  passed; the original failure and baseline records were not relabeled.
- Fresh-clone `cargo nextest run --locked`: **497 passed, zero skipped**.
- The protected import PR's complete required CI passed, including the Python
  scripts, three-platform tests, measurement comparison and release contracts:
  [run 38358858096](https://github.com/musutrade/GateHouse/actions/runs/38358858096).
- Old source main acceptance:
  [run 38358657779](https://github.com/musutrade/Harness-Gate/actions/runs/38358657779).
- GateHouse import main acceptance:
  [run 38360360232](https://github.com/musutrade/GateHouse/actions/runs/38360360232).

Both linked main CI runs completed successfully, including all three platform
performance baselines.

## Installation and protection

A fresh isolated public v0.4.8 installation passed both exact legacy Sigstore
identity checks and checksums. Binary SHA-256:
`888fa1527eae37e3163aac2e1479e91ec03a0c8380b3c49da6e9266575597418`.
`--version`, generic `init`, `config check` and `verify --all` passed.
Old/new version routing and signer rejection are covered by release contracts.
The future GateHouse signer was tested with mock certificates only; no new core
release or actual GateHouse signing ceremony is claimed.

Main requires a PR, current base and Required Quality Aggregate, with no
force-push/deletion or bypass. Immutable `v*` tags, immutable Releases and the
same core release reviewer (administrator bypass disabled) are configured.
Thresholds, measurement series, debt/ratchet, fixture identity and required
platforms are unchanged. Bootstrap alone explicitly compares to the recorded
accepted legacy source; normal future changes use their actual event base.

## Retained work and historical archive

Harness-Gate retains old commits, issues, releases and signing identities.
The only pre-existing open PR, baseline candidate #346, remains unmerged.
[GateHouse issue #2](https://github.com/musutrade/GateHouse/issues/2) carries its
review forward. Its exact original Actions archive was preserved in a separate
immutable Release and independently downloaded/hash-verified; it was not
adopted as a baseline. Core's first future GateHouse binary publication still
requires its normal release approval and real signing/provenance acceptance.
