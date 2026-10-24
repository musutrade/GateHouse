# Rust measurer 0.4.0 production consumption

Status: **local Core candidate only**; not merged, not a production pin until
this candidate is reviewed and merged. The published GateProbes artifact is
immutable and already verified.

## Fixed release identity

| Field | Value |
| --- | --- |
| Release | `rust-measure-v0.4.0` |
| Source commit | `d47afd65254f310f3f34c2bb548773eb9f31b121` |
| Archive | `harness-gate-rust-measure-0.4.0-linux-amd64.tar.gz` |
| Archive SHA-256 | `d3015acd152dfcc8ac53ee1aa653454aa51244a94ca97645bccf6cd2db6bf126` |
| Binary SHA-256 | `8770863b6e051f42428babac48fb04e58a688b29a32ae233a8cb1faf7b2ac0f2` |
| SBOM SHA-256 | `024377cad47b1f184277912db03d58151d7efc64bb290fb711f907469d567bb6` |
| Inventory SHA-256 | `3a5aafa2baa3ea41fa5be59ecd5521621779be1b0d035c445e619b3f57d755e3` |
| Exact tag identity | `https://github.com/musutrade/GateProbes/.github/workflows/collector-release.yml@refs/tags/rust-measure-v0.4.0` |

`tools/quality/rust-measure-release.json` pins this exact release. The download
path verifies the archive digest, `gh attestation` provenance, release inventory
source/tag, archive members, executable digest and capability manifest before
measurement. No cached archive or manifest marker is trusted without re-checking.

## Production consumer

`tools/quality/source_measure.py` now has the single production identity:

- `SERIES["analyzer"] == "harness-gate-rust-measure/0.4.0"`
- `MEASURER_IDENTITY["version"] == "0.4.0"`
- the same `mccabe-rust-3/1`, instrumentation, mapping, selection and compiler
  configuration dimensions as before.

`source_measure.ast` accepts only the exact 0.4.0 identity. There is no
old/new fallback, environment switch or CLI bypass. The previous 0.3.1
identity, unknown versions, a wrong rule and a wrong target echo are rejected
by the same consumer check.

`tools/quality/tests/rust_measure_candidate_acceptance.py` is the explicit
acceptance entry. It runs the released 0.4.0 binary and a real 0.3.1 binary and
checks recursive `cfg`, `cfg_attr(test, derive(...))`, exact symbol names,
exact excluded span `[[3, 1, 4, 17]]`, target echo, rejection of the real
0.3.1 binary, and fake unknown-version/wrong-rule/wrong-target identities.
It is not part of default unittest discovery.

```sh
python3 tools/quality/tests/rust_measure_candidate_acceptance.py \
  --binary /path/to/released/harness-gate-rust-measure \
  --legacy-binary /path/to/0.3.1/harness-gate-rust-measure
```

## Series and baseline impact

The analyzer version is part of the measurement-series identity. Existing
0.3.1 reports, quality baselines, retained evidence and cache entries are not
compatible with the 0.4.0 series and must not be relabeled or adopted
automatically. Re-collect both base and head under the new series and review
the resulting debt/ratchet evidence before adopting any baseline. No baseline,
debt, ratchet, threshold, requiredness or acceptance record is changed by this
candidate. See `docs/quality/rust-measure-0.4.0-series-impact.md` for the
inventory and proposed migration.

## GP contract reconciliation

GateProbes already pins the reviewed Core candidate source
`d76a3130a13a565643f5b9fda8f5a5f2b7d28453` and its
`tools/quality/source_measure_candidate.py` / acceptance files. That fixed
source is immutable for the GP release contract and is not changed by this
Core candidate. After this Core candidate merges, GP should update its
`plugins/rust-measure/candidate-core.json` to the new Core commit and its file
list, remove the now-deleted candidate module, and keep the same acceptance
path/semantics. Until that GP update is reviewed, the old `d76a313` fixed
source remains the GP contract reference; it must not be treated as the new
Core production pin.
