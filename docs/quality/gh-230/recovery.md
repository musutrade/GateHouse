# GH-230 native-input recovery diagnostics

> Storage update (2026-10-13): historical raw logs/bundles are available through
> the [verified archive guide](archive.md). Dates, digests and outcomes below are unchanged.

Status: **incomplete; no acceptance or delivery declaration**. This record
supersedes the current-status statements in the original [blocker](blocker.md)
and [validation](validation.md); those original failures are preserved unchanged.
No P6/P7 task is checked, no supported combination is added, and no production
code or policy semantics are changed. These are fresh diagnostics, not completion
of the generic invocation integration.

## Baseline and inputs

Work remained in the assigned GH-230 checkout on `symphony/GH-230`, based on
`e24aeb351168610c00c14032a473fad61659ec2b`. GH-229 was closed as completed and
PR #235 merged; its [controller acceptance](https://github.com/musutrade/Harness-Gate/issues/229#issuecomment-5630432249)
was verified before resuming. No other workspace captures were used.

The operator-provisioned official rustc-dev 1.97.1 archive has SHA-256
`0109304e1995cce9e3362208f5d4ec0944e52a2ddfbc0a85d4ce5bea5d3081ab`.
The fresh operator driver and bootstrap overlay under
`target/gh-230/operator-native-runtime/` were inventoried and assembled using
`tools/quality/build_rust_collector.py`. Exact input identities, bootstrap record,
native test log, command arrays and output are in the
[recovery archive](https://github.com/musutrade/Harness-Gate/blob/d9a932b23dcfea709175285b594ca200450a9971/docs/quality/gh-230/recovery-evidence.tar.gz.parts/manifest.json). This resolves the original missing
archive blocker. The archive was not downloaded again.

## Fresh native observations

An installed private runtime captured the original `driver_complete.rs` fixture
and performed two complete retained-binary re-exports. Landlock ABI 8 denied
reads of the checkout source and execution of ambient Python; both denials were
asserted. The private runtime, dedicated capture directory and inventoried host
runtime libraries were allowed. The diagnostic runner and all original captures,
anchors, binaries, raw profiles, reports and replay files are retained in the
recovery archive without rewriting embedded paths or trust anchors.

The result had line coverage 26/35 and `legacy_debt` CRAP 56/1. The returned
measurement omitted threshold verdict fields. One compiler launch and one
sample producer generated the capture; three native exports covered the initial
export and both re-exports. Wall times were 1.1658 seconds for capture, 0.7460
seconds for first re-export and 0.7310 seconds for the second.

This existing host was not a fresh OS installation. Landlock did not isolate
filesystem metadata or all network protocols; TCP bind/connect were denied.
`bwrap` failed with `No permissions to create a new namespace`. The first
Landlock runner failed private Python startup because its incorrect `argv[0]`
prevented encodings discovery. The corrected absolute executable invocation
passed. Both failures and both runner versions remain retained.

Five existing opt-in native tests passed in two runs (one test in 10.088 seconds,
four tests in 86.569 seconds): low-coverage completion with preserved legacy exit,
doctor/unknown collection without sampling, wrong-anchor rejection, complete
classification, and actual Rust Core policy evaluation. The latter used the
checkout-built 0.3.7 executable, **not the distributed release binary**, despite
the existing test method's name. Test capture directories with the historical
`target/gh-228/runtime-tests/` prefix are inside this assigned GH-230 checkout;
their fresh original bytes are retained in the archive.

Three runtime archives were byte-identical: 1,623,470,080 bytes each, SHA-256
`ede2f68062b3a7ceb9c324fb7066b9c66c7af5a35adf489a41366b14d0850b41`.
Assembly into a fresh directory took 42.7644 seconds; a subsequent new-directory
assembly took 28.3319 seconds. Sampled peak allocated output space (directory plus
tar, 0.2-second sampling) was 3,248,439,296 and 3,248,431,104 bytes respectively.
These exclude prebuilt inputs and existing artifacts. OS caches were not cleared;
these are not cold-host installation measurements. The full runtime archives
remain workspace-local; the recovery archive retains their identity/inventory,
not their complete 1.6 GB payload. Durable acceptance is therefore not certified.

## Released-Core prerequisite

The exact command attempted was:

```sh
curl --fail --location --max-time 30 https://github.com/musutrade/Harness-Gate/releases/download/v0.3.7/harness-gate-linux-amd64 --output target/gh-230/recovery/released-core
```

The execution tool rejected access: `Network access to "https://github.com:443"
was blocked by policy.` It returned no process exit code; retained curl stderr
also reports HTTP 403. No binary was downloaded. The available global Core is
0.1.0. A checkout-built 0.3.7 does not establish the released binary's identity.

To unblock that acceptance prerequisite, provision the original
`harness-gate-linux-amd64` v0.3.7 asset in this workspace, with its release
verification material. GitHub release metadata reports asset ID 542733258,
8,716,944 bytes, SHA-256
`7b148bd293acaff6a58fb15ac5e44929b5a2fe9232a5acc658d02883c3a79ff3`.
The [release metadata](https://github.com/musutrade/Harness-Gate/blob/d9a932b23dcfea709175285b594ca200450a9971/docs/quality/gh-230/recovery-release-metadata.json) records asset digests and
URLs; metadata alone is not binary execution or signature verification.

## Local validation and remaining work

With workspace-local `CARGO_TARGET_DIR=$PWD/target/gh-230/cargo`,
`TMPDIR=$PWD/target/gh-230/tmp` and
`GIT_CEILING_DIRECTORIES=$PWD/target/gh-230/tmp`, the fresh required checks were:

- `cargo nextest run --manifest-path tools/harness-gate/Cargo.toml --locked`:
  exit 0, 392 passed, none skipped. Proxy variables were removed for local
  loopback tests. The original failed run remains retained separately.
- `cargo fmt --manifest-path tools/harness-gate/Cargo.toml -- --check`: exit 0.
- `cargo clippy --manifest-path tools/harness-gate/Cargo.toml --all-targets -- -D warnings`:
  exit 0.
- `python3 -m unittest discover -s tools/quality/tests -v`: exit 0, 402 tests,
  three skipped native classes. Those skips are not acceptance evidence; the
  five explicit native tests above ran separately with private-runtime inputs.
- Documentation consistency, strict OpenSpec and whitespace results are recorded
  in [recovery checks](https://github.com/musutrade/Harness-Gate/blob/d9a932b23dcfea709175285b594ca200450a9971/docs/quality/gh-230/recovery-checks.json), with their complete logs archived.
- `harness-gate config check` and `harness-gate verify --profile ci --all`:
  not applicable because `.harness-gate/flow.toml` is absent.

Generic trusted invocation, its complete Core/capability/producer-count matrix,
all requested acceptance negatives, released-Core evaluation, clean-host cold
installation and durable package retention remain incomplete. Synthetic tests
and this fixture do not certify Arc-Admin. No baseline was accepted, no release
was published, and no #215 integration was enabled. Required CI and controller
review have not run for this unfinished work. No PR or handoff declaration was
issued.
