# Rust measurer 0.4.0 series and baseline impact

This is the pre-merge inventory for the local Core consumption candidate. It
records what binds the Rust measurement series and what must be re-collected;
it does not adopt, reset or relabel any baseline.

## Series-bound production inputs

| Location | Binding | Effect of 0.4.0 |
| --- | --- | --- |
| `tools/quality/source_measure.py` | `SERIES` and `MEASURER_IDENTITY` | Analyzer dimension changes from `harness-gate-rust-measure/0.3.1` to `/0.4.0`; all other dimensions stay unchanged. |
| `tools/quality/rust-measure-release.json` | exact release, archive/binary/SBOM identity, source commit, tag certificate | Points at immutable `rust-measure-v0.4.0`; no floating tag or fallback. |
| `tools/quality/measurer_release.py` | capability identity and digest-specific download cache | Accepts only the pinned 0.4.0 manifest/binary; cache is namespaced by archive SHA-256. |
| `tools/quality/ci_quality.py` risk stages | report `series`, `configuration`, `tools`, `source_inventory`; base/head comparison | Both sides must use the same new series/configuration/tools; stale 0.3.1 reports fail closed. |
| `tools/harness-gate/src/config/quality/baseline.rs` | baseline state/profile/tool/measurement-series compatibility | An existing 0.3.1 baseline cannot be used as a 0.4.0 baseline. |
| `tools/harness-gate/src/verify/quality.rs` | baseline request/evaluation and retained evidence | A required baseline must be resolved for the new series; no automatic adoption. |
| `tools/harness-gate/quality-core/ratchet.rs` and policy schema | debt/ratchet decisions over supplied base/head values | Thresholds, requiredness and debt semantics are unchanged; only the series-bound evidence must be re-collected. |
| `docs/quality-baselines.md` and `.github/workflows/quality-baseline-refresh.yml` | trusted baseline workflow and retained-artifact rules | The refresh workflow captures benchmark candidates; it does not adopt a risk baseline or relabel old evidence. |

## Current repository baseline state

- The repository quality preset (`tools/harness-gate/presets/quality-base.json`)
  has `baseline.required: false` and provider `none`. There is no checked-in
  project baseline to migrate in this repository.
- The checked-in Rust fixtures under `tools/quality/fixtures/` and
  `docs/dogfood/` are test/dogfood inputs, not production baselines. Historical
  series fixtures must not be rewritten to make the new series pass.
- The required CI risk stage produces a review-only candidate and explicitly
  states it is not an accepted baseline.
- No `target/quality` or `target/rust-measure-release` cache is committed.
  Caches are disposable and are re-verified by digest/provenance on use.

## Migration proposal

1. Keep all historical 0.3.1 reports, baseline bundles, acceptance records and
   cache entries intact for audit/rollback; do not reinterpret or rename them.
2. Re-collect both base and head with the new 0.4.0 series and the same source
   selection/configuration. Compare the new pair and review changed symbols,
   excluded spans and debt/ratchet outcomes.
3. If a project has a required baseline, resolve a fresh baseline for the new
   series through the trusted host workflow, with an externally authenticated
   manifest and compatible profile/tool/series state. Do not edit an old
   baseline's series string.
4. Only after review, adopt the new baseline/threshold/requiredness through the
   normal policy change process. This candidate changes none of them.
5. Keep the old `rust-measure-v0.3.1` pin available only as historical
   rollback evidence; production `source_measure.ast` has no old/new fallback.

## Bounded same-input comparison

The explicit acceptance command runs the released 0.4.0 binary and a real 0.3.1
binary on the same recursive-`cfg` fixture. The 0.4.0 consumer accepts the new
symbol set and exact span; the production consumer rejects the 0.3.1 identity.
The old 0.3.1 consumer is not retained as a fallback in this candidate. Raw
comparison output is retained outside the repository with the candidate result.
