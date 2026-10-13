# GH-230 malformed configured-output continuation

> Storage update (2026-10-13): historical raw logs/bundles are available through
> the [verified archive guide](archive.md). Dates, digests and outcomes below are unchanged.

Status: partial diagnostic; P6/P7 remain incomplete. All relevant Engineering
Policy semantics remain unchanged.

The distributed Core v0.4.0 now has an additional generic configuration test for
malformed collector output. A disposable signed request launches a synthetic
shell producer that records its invocation, prints incomplete JSON, and exits
zero. Core rejects it with `malformed adapter response`, creates no collection
report or evidence artifacts, and consumes the nonce. Replaying the request is
rejected before another invocation. The external counter contains exactly one
launch after both calls.

This tests the real released Core's response parsing and replay behavior. The
malformed producer is deliberately synthetic; it does not establish successful
native collection, absence of all fallback processes, or descendant producer
counts. The existing configured installed-collector rejection test also remains
passing. Neither test changes the empty reviewed compatibility matrix.

`python3 -m unittest test_rust_collector_runtime.StandaloneNativeTests -v` passed
all ten tests with zero skips (exit 0, 84.633 seconds wall time). This run creates
fresh local native captures and uses the staged official Core binary with SHA-256
`8e3df8303ca8f650d4ef768b29cfefb60ca115122a47b116245cc19e4649bbdd`.
It uses the complete environment from the preceding corrected discovery,
including workspace-local build/capture directories and the existing private
Cargo vendor directory. There were no failures in this new test run.

The [command records](https://github.com/musutrade/Harness-Gate/blob/d9a932b23dcfea709175285b594ca200450a9971/docs/quality/gh-230/malformed-core-checks.json) retain exact commands,
environments and results. The [validation archive](archive.md)
contains original output and tested source snapshots, with a
[verified inventory](https://github.com/musutrade/Harness-Gate/blob/d9a932b23dcfea709175285b594ca200450a9971/docs/quality/gh-230/malformed-core-validation-archive.json).
The [native evidence archive](https://github.com/musutrade/Harness-Gate/blob/d9a932b23dcfea709175285b594ca200450a9971/docs/quality/gh-230/malformed-core-evidence.tar.gz.parts/manifest.json) retains the new
capture/re-export bytes, signed fixture inputs and individual process records;
its [inventory](https://github.com/musutrade/Harness-Gate/blob/d9a932b23dcfea709175285b594ca200450a9971/docs/quality/gh-230/malformed-core-inventory.json) and
[receipt](https://github.com/musutrade/Harness-Gate/blob/d9a932b23dcfea709175285b594ca200450a9971/docs/quality/gh-230/malformed-core-archive.json) verify every retained member's SHA-256.
These local native bytes are diagnostic evidence, not clean-host acceptance.

Only test code changed in this continuation. The preceding full Python run
(434 passed, zero skips) and Rust validation results remain in the
[configured Core record](configured-core-continuation.md); they are not new runs
or a claim that full discovery covered the newly added test. Documentation
consistency, strict OpenSpec and whitespace checks are recorded alongside the
new targeted run. Repository-level `harness-gate config check` and
`harness-gate verify --profile ci --all` remain not applicable because this
checkout has no `.harness-gate/flow.toml` declaring a `ci` profile.

The [recorded Docker socket permission denial](container-access-recheck.md)
still blocks the operator-provisioned clean-host infrastructure. No changed
sandbox access was supplied, so the same probe was not repeated. The remaining
acceptance work requires that access: installed positive generic invocation,
complete clean-host native re-export, observed producer counts, cold/warm and
peak-disk measurements, and evidence sufficient to freeze a supported tuple.
No checkbox, acceptance PR, or completion handoff is declared. Existing edits
and genuine failure records remain intact; there is no release or Arc-Admin action.
