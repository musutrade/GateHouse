# External regression data

Ordinary `cargo build --manifest-path tools/harness-gate/Cargo.toml --locked`
uses only committed source and small fixtures. It does not fetch evidence.

Before full Rust/Python regression or documentation validation, run:

```sh
python3 tools/maintenance/test_data.py
cargo test --manifest-path tools/harness-gate/Cargo.toml --locked
python3 tools/quality/tests/run_core.py
```

`--list` shows the three groups and download sizes; `--group NAME` selects one.
The default prepares all groups (about 35.1 MiB compressed). Each group is cached
by SHA-256 in `~/.cache/gatehouse-test-data`; `--cache PATH` changes the location.
`--offline` uses existing files or the verified cache and fails on any cache miss.
Copy that cache directory to an offline machine before running full tests.

The committed manifest fixes the original source commit, Release URL, archive
size/hash, and every member size/hash. No `latest` URL is used. Existing changed
files are never overwritten; resolve local edits explicitly. Archive validation
rejects traversal, links, missing/extra members and digest changes. Restored
paths are individually ignored by Git. Run preparation after changing branches:
changed retained data fails rather than silently substituting a different version.
For upgrades, review the new immutable archive and regenerate the manifest and
exact ignore list in the same PR. Keep old Release assets for older checkouts.

GitHub Actions prepares data only in jobs that consume it. Build, format, lint
and security jobs require no evidence download. Quality measurement snapshots
restore the **measured commit's own manifest**, independently for base and head.
No threshold, measurement series, debt baseline, required job or release authority
changes. A download/verification failure fails the consuming job and aggregate.

Historical log/report links point to the preserved original source revision;
no historical failure is rewritten. Large datasets remain owned by their
original core/plugin owner: external storage is not an ownership transfer.
