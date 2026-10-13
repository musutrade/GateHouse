# Rust equivalence acceptance window

GH-118 scopes acceptance to retrospective projection/policy equivalence on the
two existing retained Rust collection runs below. Both distinct head commits
and both distinct run IDs are required. Repeating one run does not satisfy the
window; commits between these points are not claimed as additional observations.
No hosted soak duration is claimed. Live CI starts an ongoing advisory shadow
comparison with this PR.

| Retained run | Base | Head |
| --- | --- | --- |
| `gh96-local-3` | `c0e612351c2efd515fde96bcefad80c1365534a2` | `41e23761e0caa06cab1f6d5ebca1a79648fbb301` |
| `gh97-final-local` | `381d5f418655969d29b81f89069845e6667f35c9` | `381d5f418655969d29b81f89069845e6667f35c9` |

The [window manifest](../../tools/quality/fixtures/rust-reference/equivalence-window.json)
pins archive hashes, candidate hashes, required counts and original identities.
The [GH-96 archive](https://github.com/musutrade/Harness-Gate/blob/d9a932b23dcfea709175285b594ca200450a9971/docs/quality/gh-96/candidate.tar.gz) and
[GH-97 archive](https://github.com/musutrade/Harness-Gate/blob/d9a932b23dcfea709175285b594ca200450a9971/docs/quality/gh-97/candidate.tar.gz) remain unchanged. The first run supplies
distinct base/head snapshots; the second supplies a same-commit comparison.
Both retain native historical debt and unsupported branch coverage.

Run the complete window, including adversarial fixtures, in a fresh directory:

```bash
python3 tools/quality/rust_equivalence.py --output target/quality/rust-equivalence
```

The runner returns nonzero unless both pinned replays and all adapter fixtures
pass without skips. `acceptance.json` records evaluator file digests, each run,
all failures and fixture results. Each run retains its original candidate plus
normalized production/base-risk/head-risk evidence, project and policy JSON,
and `shadow.json`. Raw source and measurement hashes remain linked to the
original candidate. Candidate files are never rewritten or promoted to baselines.

Acceptance requires no unexplained differences in production gate outcomes,
all function identities and debt classifications, aggregate state, unavailable
capabilities, or measurement-error semantics. There is no mismatch allowlist.
Missing or duplicate policy results and an optional unsupported gate changing
to success also block acceptance. A missing/modified artifact, stale identity,
unknown series or malformed numeric value produces `measurement_error`.

The [adapter fixtures](../../tools/quality/tests/test_rust_reference.py) additionally
exercise threshold failure with unchanged raw counts, low/high-complexity changed
functions, historical debt, injected policy disagreements, missing and modified
raw evidence, stale run/raw provenance, duplicate/nonfinite JSON, invalid exact
values, unsupported-state drift, omitted policy results, missing candidates and
failed/cancelled/skipped/measurement-error required stages. Derived fixtures are
explicit test inputs, never additional historical runs. Non-pass required stages
retain their original state while both aggregates block delivery.

The [validation record](gh-118-validation.md) records the completed local window
and its retained results. This is compatibility acceptance for these retained
Rust contracts, not a new accepted production baseline or authority switch.
[ADR-0040](../adr/0040-language-agnostic-evidence-policy.md) requires a separate
rollout decision before replacing any current required gate.

## Live shadow contract

`Generic Quality Shadow` depends only on `quality-coverage`, uses `always()` and
downloads `quality-coverage-<run_id>-<run_attempt>` from the same workflow run.
Both jobs receive the event base, `github.sha` (including the PR merge commit),
and run/attempt identity. Projection cannot silently fall back to another run.
Upload retains `quality-generic-shadow-<run_id>-<run_attempt>` JSON reports even
on failure. Keep both artifacts together when investigating a comparison.

The shadow job installs Python only and never invokes collection. A missing
download remains a failing job; the projection step still emits a structured
missing-candidate error where the runner is available. Runner loss cannot be
represented as successful acceptance.

`Required Quality Aggregate` and its existing 15 dependencies remain unchanged.
The shadow job is excluded from that dependency list, while its own failures are
visible. Required CI must pass before merge. The controller owns CI observation,
merge and issue closure; GH-118 submission does not claim hosted CI success.
