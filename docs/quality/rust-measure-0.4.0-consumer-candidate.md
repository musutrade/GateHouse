# Rust measurer 0.4.0 consumer candidate

Status: **local candidate only**; not a release pin, not merged, no gate change.

Base: `7a5fd51d781340312c73cc74e3257bb3dfd28404` (current `origin/main`).
The local `main` checkout was stale at `47d8d86`; this candidate uses the
verified `origin/main` base. Production pin remains `rust-measure-v0.3.1`.

## Scope

- `source_measure.py` keeps the production default identity and `SERIES` at
  `harness-gate-rust-measure/0.3.1` / `mccabe-rust-3/1`. Its AST call is
  factored through `ast_with_identity` but `ast` still requires exactly the
  old identity.
- `source_measure_candidate.py` is an explicit 0.4.0 consumer identity/series:
  `CANDIDATE_IDENTITY` and `CANDIDATE_SERIES`. It is not selected by an
  environment variable, CLI flag or user-controlled switch.
- `tools/quality/tests/rust_measure_candidate_acceptance.py` is an explicit
  acceptance command, not part of default discovery. It runs the candidate
  against a real 0.4.0 binary and a real 0.3.1 binary, checks exact excluded
  spans and target echo, and rejects unknown version, wrong rule and wrong
  target echo.

## Acceptance

```sh
python3 tools/quality/tests/rust_measure_candidate_acceptance.py \
  --binary /path/to/harness-gate-rust-measure \
  --legacy-binary /path/to/harness-gate-rust-measure-0.3.1
```

The command checks:

- recursive `cfg` production view (`any(test, unix)`, `all(test, target_os)`,
  `all(unix, not(test))`, `cfg_attr(test, derive(...))`);
- symbol names and exactly `[[3,1,4,17]]` excluded spans; a mutated span
  response is rejected by the same check;
- exact target configuration echo;
- candidate series `harness-gate-rust-measure/0.4.0`;
- production `source_measure.SERIES` remains `0.3.1`;
- the old 0.3.1 consumer accepts the real 0.3.1 binary on a plain fixture and
  rejects the 0.4.0 binary;
- the candidate accepts the 0.4.0 binary and rejects the real 0.3.1 binary by
  identity;
- fake responses with unknown version, wrong rule and wrong target echo are
  rejected. These are identity/echo negatives, not a schema-validation claim.

## Compatibility chain

The active pin and release contract are intentionally unchanged. The staged
path is:

1. GP release gate keeps the old-Core contract as an expected negative and
   adds a fixed reviewed Core candidate contract (this candidate's source) as
   the positive; no disposable identity patch is accepted as release evidence.
2. GP publishes the immutable 0.4.0 artifact only after both expectations and
   downloaded-asset acceptance pass.
3. A separate GH#108 Core PR updates the active pin/series and runs the
   consumer contract against the downloaded GP archive; old baselines are not
   relabeled.
4. The GP contract is rerun against the released Core before closure.

The collector 0.2.0 contract is separate: Core currently has no active
production collector pin (`rust-collector-compatibility.json` is `tested: []`),
and a successful `discover`/`collect` is not by itself full Core consumption.
