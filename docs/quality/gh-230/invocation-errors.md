# GH-230 generic operational-error follow-up

> Storage update (2026-10-13): historical raw logs/bundles are available through
> the [verified archive guide](archive.md). Dates, digests and outcomes below are unchanged.

Status: incomplete P6/P7; no supported combination, acceptance PR or handoff.
This follows [project integration](project-integration.md). Prior diagnostic
records and evidence archives are unchanged.

The private entry previously ran runtime validation before reading the generic
request. An unsupported ABI or changed private tool therefore returned the
legacy developer response instead of the project collector response with the
invocation ID. The entry now reads the envelope first, preserving generic
operational errors without launching native certification or sampling. Core
request authentication and final policy decisions remain unchanged.

A new regression test covers both errors and requires an exact generic FAIL
response, matching invocation ID, empty artifacts/evidence, zero native calls
and no output files. Running it against the original entry retained in
`project-evidence.tar.gz` reproduced two assertion failures. This is a synthetic
transport regression, not a native acceptance measurement. Exact commands and
original failure/success logs are retained in the companion evidence archive.

The previously measured runtime archive predates this follow-up; its recorded
hash and costs do not describe the changed entry. No current runtime package or
supported tuple is certified by this check.

The operator's local rustc-dev archive and native driver resolve bootstrap.
The provisioned-input directory still contains only the rustc-dev archive.
The original released v0.3.7 Core asset and verification material remain missing;
the previously policy-blocked network download was not retried. Its exact asset
identity and provisioning requirement are in
[recovery](recovery.md#released-core-prerequisite). Clean supported-host positive
installation, cold/warm measurements and durable complete package/capture
retention also remain outstanding. Authenticated generic positive invocation and
the full actual-Core capability/producer matrix remain incomplete. No task
checkbox was changed, and no release or Arc-Admin action was taken.

Validation: `python3 -m unittest discover -s tools/quality/tests -v` passed
(408 tests, three explicit opt-in skips; 179.020 seconds). Earlier Rust checks
remain applicable because no Rust source or Cargo inputs changed. Project-local
`config check` and `verify --profile ci --all` remain not applicable: the checkout
has no `.harness-gate/flow.toml`. Required hosted CI remains pending.

[Exact check results](https://github.com/musutrade/Harness-Gate/blob/d9a932b23dcfea709175285b594ca200450a9971/docs/quality/gh-230/invocation-error-checks.json),
[original logs and changed sources](archive.md),
[file inventory](https://github.com/musutrade/Harness-Gate/blob/d9a932b23dcfea709175285b594ca200450a9971/docs/quality/gh-230/invocation-error-inventory.json) and
[archive receipt](https://github.com/musutrade/Harness-Gate/blob/d9a932b23dcfea709175285b594ca200450a9971/docs/quality/gh-230/invocation-error-archive.json) retain this follow-up separately
from previous evidence. Strict OpenSpec, documentation consistency and whitespace
validation results are included in the check results.

The subsequent [authenticated invocation diagnostic](trusted-invocation.md)
records actual Core signature and replay checks with a fresh private runtime.
