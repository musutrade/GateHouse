# GH-230 generic projection progress

> Storage update (2026-10-13): historical raw logs/bundles are available through
> the [verified archive guide](archive.md). Dates, digests and outcomes below are unchanged.

Status: **incomplete; no acceptance, supported tuple, PR or handoff declaration**.
This continues the [native-input recovery diagnostics](recovery.md), which remain
unchanged. No P6/P7 checkbox is complete. All relevant Engineering Policy semantics
remain unchanged; no frozen oracle or Rust Core dispatch code is modified.

## Implementation boundary

`rust_collector_project.py` validates the existing generic adapter v2/project
collector v1 envelope and a digest-pinned capture binding carried in the signed
arguments. It checks invocation/context, project, selected owner/metric/series
claims, native identity and capture anchor. Core remains responsible for request
signature/replay checks and all policy decisions. This module does not sign or
approve requests, accept capture anchors, or accept a baseline.

The projection produces typed counts, coverage ratios and exact rational CRAP,
with all six capability states preserved. Legacy verdict fields are excluded
from projected raw facts without modifying original native reports. Native JSON
is an opaque artifact; its tool versions can contain newlines and its legacy
presentation values can be floating point. Their identity is digest-bound rather
than inserted into the stricter normalized evidence JSON domain. The normalized
metrics still contain exact integers and rational values only.

The private package includes this module and its strict binding schema. The
public `collect` entry validates a generic request and returns a generic error
response for an unknown delivery combination. It still refuses all collection:
the reviewed compatibility matrix is empty. The projection is an internal stage,
not a working installed-host generic invocation. Delivery authentication,
observed-environment preflight, native re-export and generic positive collection
still need to be connected and tested together before P6.1 can be completed.

## Diagnostics and genuine failures

Five synthetic transport tests passed. They cover low-coverage completion,
unsupported/not-configured/not-collected/not-applicable/measurement-error states,
tampered bindings, stale context, incomplete claims, changed sources/native
identity, missing owners and zero native calls on unknown delivery combinations.
These negatives establish no native positive acceptance.

The first fresh native projection test failed with
`MeasurementError: invalid character in evidence string`: multiline native tool
versions were incorrectly passed to normalized evidence canonicalization. The
original failing run, its capture bytes and stderr are retained. The fix keeps
native JSON opaque and pins its identity by digest; it does not change the frozen
`harness_evidence.py` JSON domain. The first full Python run also failed because
the new production module was missing from the Python boundary inventory; a
second already-running suite repeated that failure. Both logs remain retained.
Both inventories now include the measurement module, and changed A/B module
hashes are updated; frozen C-module hashes and semantics remain unchanged.

Two subsequent fresh native runs passed (17.375 s and 21.952 s). The latter also
exercised the installed public entry's unknown-combination rejection. Exact
commands and environment are in the retained `native.command.json` and
`native-entry.command.json` records. Both runs compiled and sampled fresh base
and head fixtures, certified their retained captures, projected exact CRAP 56/1,
and obtained an actual Core policy failure without a measurement error.

The native test deliberately uses `HARNESS_GATE_NATIVE_POLICY_BINARY` pointing to
the checkout-built Core. Its request preparation is unsigned test data. It tests
the installed projection stage followed by actual Core `quality evaluate`; it
does not claim Core authenticated that preparation or invoked the public
collector successfully. The test chooses the fixture's `legacy_debt` function
for the existing absolute CRAP rule. No business test or normative threshold is
changed.

## Retention and validation

Exact commands, exit codes, timing and original stdout/stderr are indexed in
[project-checks.json](https://github.com/musutrade/Harness-Gate/blob/d9a932b23dcfea709175285b594ca200450a9971/docs/quality/gh-230/project-checks.json). The archive
[project-evidence.tar.gz](https://github.com/musutrade/Harness-Gate/blob/d9a932b23dcfea709175285b594ca200450a9971/docs/quality/gh-230/project-evidence.tar.gz.parts/manifest.json) includes all three new native
test work directories (including the failed run), binaries, raw profiles,
re-export outputs, request bindings, Core reports, both installed adapter source
versions and validation logs. [project-inventory.json](https://github.com/musutrade/Harness-Gate/blob/d9a932b23dcfea709175285b594ca200450a9971/docs/quality/gh-230/project-inventory.json)
records every retained file's size and SHA-256;
[project-archive.json](https://github.com/musutrade/Harness-Gate/blob/d9a932b23dcfea709175285b594ca200450a9971/docs/quality/gh-230/project-archive.json) identifies the archive itself.
The historical `target/gh-228/runtime-tests` prefix belongs to the test harness:
these three directories were freshly created inside this GH-230 workspace.

Final `python3 -m unittest discover -s tools/quality/tests -v` passed: 407 tests,
three explicit opt-in skips, 179.877 seconds. The new native test passed separately
with the private runtime enabled. Strict OpenSpec validation, documentation
consistency and `git diff --check` passed. Earlier failed Python runs and the
native projection failure are included alongside the successful results.

The current diagnostic package is 1,623,480,320 bytes, SHA-256
`59a79edb22c4c8f1294561ff1f634295eb38170d837698bc4f4652aac1ddf01c`.
It remains at `target/gh-230/project-progress-v2/runtime.tar`; full runtime
binaries are excluded from the small evidence archive. The new package therefore
has no durable complete-package acceptance receipt. Inventory/assembly took
22.145/18.652 seconds on this already-used host. These are diagnostic assembly
times, not cold/warm clean-host installation costs or peak-disk measurements.

The previous [Rust checks](https://github.com/musutrade/Harness-Gate/blob/d9a932b23dcfea709175285b594ca200450a9971/docs/quality/gh-230/recovery-checks.json) remain applicable: nextest
(392 tests), formatting and clippy passed. No Rust source or Cargo input changed.
`harness-gate config check` and `harness-gate verify --profile ci --all` are not
applicable because `.harness-gate/flow.toml` is absent. Required hosted CI remains
pending; there is no acceptance PR or declaration for unfinished P6/P7 work.

## Remaining acceptance prerequisites

The operator archive and freshly built driver resolved the original bootstrap
blocker. The independently distributed v0.3.7 Core asset is still unavailable in
this workspace; the policy-blocked download, exact asset digest and required
provisioning are recorded in [recovery](recovery.md#released-core-prerequisite).
No blocked network route was retried. An existing-host private-runtime diagnostic
is also not a fresh supported-host installation or a cold-cache measurement.

P6 remains incomplete until authenticated generic invocation and actual Core
capability/producer behavior are exercised together. P7 remains incomplete until
clean-host positive collection/re-export, its negative matrix, measured cold/warm
costs and durable complete package/capture bytes establish an exact supported
tuple. Existing projection JSON is not a replacement for retained native binaries
and profiles. Package relocation is not measurement-series equivalence; signatures
of distribution files do not authenticate runtime captures. Arc-Admin/#215,
release publication and baseline acceptance remain outside this work.

The subsequent [generic operational-error follow-up](invocation-errors.md)
records a transport correction and validation after this archived revision.
