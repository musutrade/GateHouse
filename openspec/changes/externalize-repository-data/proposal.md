# Externalize repository data and move lean source to GateHouse

## Why
Clone includes historical evidence and large frozen inputs. Local cache cleanup
does not reduce the public Git history that every complete clone transfers.

## What Changes
Pin external Release data with per-member hashes; restore it only for full
regression jobs, independently for measured base/head snapshots. Preserve source,
small build fixtures and necessary documentation. Export the accepted lean tree
into GateHouse without importing old Git history; retain Harness-Gate as archive.
Bootstrap GateHouse measurements must explicitly compare against the preserved
accepted source commit, never the placeholder README or a reset debt baseline.
Preserve CLI/crate names, legacy installations and historical signing identities.

Engineering Policy semantics, thresholds, measurement series, required gates,
ratchet/debt and release approval authority remain unchanged. The user approved
this repository move on 2026-10-13, superseding the earlier migration deferral.

## Impact
Full regression needs the pinned data or an offline cache. Basic builds do not.
Existing archives and evidence identities remain available in Harness-Gate.
