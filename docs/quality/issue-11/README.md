# GateProbes measurer consumption acceptance — Issue 11

Engineering Policy semantics are unchanged. The collector remains measurement-only;
Core retains decisions, requiredness, thresholds, historical baseline/debt/ratchet
and release authority. No required gate, source boundary or waiver is relaxed.

## Fixed identities and continuity

The former declaration is preserved byte-for-byte in [legacy-pin-v1.json](legacy-pin-v1.json).
The active [pin](../../../tools/quality/rust-measure-release.json) declares GateProbes
`rust-measure-v0.3.1`, source `f9834a988e9e55ec99b7ebe81fb67a9ca16b2928`, exact
`collector-release.yml` tag identity and GitHub Actions OIDC issuer. Its archive
SHA-256 is `5d3130b8d6673b9e32133be88fa69e4404f1d4ecc5ae022f7abf008c9b06302f`.

[Public provenance](public-provenance.json) records successful signed SLSA
verification with exact repository, workflow certificate SAN, tag, signer SHA and
source SHA. The separately published archive signature also passed Cosign with
that exact identity. [Legacy provenance](legacy-provenance.json) records the
original archive's distinct Harness-Gate identity. [Public negative cases](public-negative.json)
reject the wrong repository and tag.

[Continuity evidence](continuity.json) compares both downloaded archives, verifies
their independently pinned archive digests, and establishes identical executable
and capability manifest bytes. The executable SHA-256 in both is
`f17ec90502005ddb161143cd5683a292fa1ccde7f4715291e0a0f82ae5b519ad`.
The report includes actual AST and complexity output comparisons for all 67
available selected source files and fixed AST fixtures under Linux, macOS and
Windows target cfgs: 201 comparisons, with matching exit status and full output.
Any equivalent unsupported case remains a failure, never a metric.

Reproduce with the two public release archives:

```bash
python3 docs/quality/issue-11/verify_continuity.py \
  --legacy-archive /path/to/legacy/harness-gate-rust-measure-0.3.1-linux-amd64.tar.gz \
  --new-archive /path/to/gateprobes/harness-gate-rust-measure-0.3.1-linux-amd64.tar.gz \
  --output /tmp/measurer-continuity.json
```

The AST engine, measurement series, closure instrumentation, LLVM join,
complexity definitions and risk/ratchet decisions are unchanged. Provenance now
records the actual repository and trust declaration; it also hashes the shared
release consumer. Base and head are measured with the same new declaration.
Historical reports keep their original provenance and are not mixed with fresh
reports or rewritten to claim the new source.

## Consumption and failures

`measurer_release.py` is the single implementation used by `Collector.measurer`
and Core measurement tests. Both freshly downloaded and cached archives must pass
archive digest and signed SLSA verification. The signed release inventory must
match the tag, repository, source commit, package name, size and digest. Extraction
checks every member before writing into a fresh temporary directory, rejects
links/traversal/duplicates and caps file count and total unpacked size. The
installed executable digest and exact supported manifest capabilities must match.
Downloads and installed binaries are isolated by archive digest, since the legacy
and independent releases share an asset filename.

Missing assets, wrong identity, unsupported analyzer/version/rule/capabilities,
corrupt cached content and unverified provenance fail. There is no latest lookup,
cross-repository fallback or temporary source build. Runtime requires the existing
GitHub CLI with attestation support and network access to release/Sigstore services;
CI supplies its read-only GitHub token to the two consuming jobs. Local callers use
their existing GitHub CLI authentication. Availability failures remain blocking.
The extra verification adds two signed provenance checks and an inventory fetch
per consumer invocation; it avoids any additional Rust compilation.

## Plugin test handoff scope

[The original Issue 10 receipt](prior-test-handoff.json) remains byte-identical.
The current receipt removes `ci_quality.py` and `source_measure.py` from the
plugin parity dependency boundary. Neither is used by the delegated test modules
or their transitive local imports, as recorded in the
[dependency audit](plugin-dependencies.json). These consumers' release and AST/risk
regressions remain required Core tests. No plugin execution evidence is renewed
or implied. All actual shared/plugin inputs retain their prior hashes; the
previously omitted `rust_collector_config_fixture.py` dependency is additionally
protected. Selection verifies the preserved receipt, audit hashes, full graph,
exact two-file scope delta and unchanged delegated execution acceptance. Negative
tests reject an added consumer dependency or the removal of another shared input.

## Validation and hosted acceptance

Local validation passed 37 existing source-measure tests, 12 release pin tests,
63 consumer/CI/retention/selection tests before the final scope tests, and the
subsequent selection regression tests. Source-measure tests exercise real compiled
fixtures, closure coverage joins, function risk and lineage comparisons with the
verified independent release. Pure documentation consistency also passed.
Raw local logs, including earlier rounds, remain under `/tmp/issue-11-*`.

The initial hosted candidate `7e7407d` ran all 536 Core Python tests and failed
one registration check: the new release helper was absent from the Python module
inventory. The original failure log remains `/tmp/issue-11-hosted-quality-scripts-failure.log`
(run `38415351493`). The correction introduces the maintained
[Python boundary inventory](../python-boundary.json), preserving historical
inventories and registering the helper as existing repository CI/dev tooling.
The module-coverage regression reads the current inventory; no test is removed
or weakened and the frozen oracle retains its original hashes.

The earlier hosted acceptance requirement is now satisfied. The exact merged
revision `189fc31c4f12167b0737eff369d64aba700ab5a0` passed
[main CI 38495521539](https://github.com/musutrade/GateHouse/actions/runs/38495521539),
including Core risk, coverage and Required Quality Aggregate. PR #17 merged
and Issue #11 closed. The original failed candidate and local evidence remain
unchanged; this status update does not replace them.

## Explicit rollback

[rollback-pin-v2.json](rollback-pin-v2.json) declares the old archive and its
original Harness-Gate tag/workflow/source identity in the new schema. Select it
explicitly by restoring that JSON as `tools/quality/rust-measure-release.json`
through a reviewed change. This restores the artifact identity while preserving
all current trust checks and policy gates; it does not reset a baseline.
The original v1 pin alone is intentionally rejected by the new loader.
The separate rollback run passed with both legacy archive and inventory verified.

A separate rollback acceptance can be run without editing the active pin:

```bash
python3 tools/quality/measurer_release.py \
  --pin docs/quality/issue-11/rollback-pin-v2.json --cache /tmp/measurer-rollback
```

This must verify the legacy archive and signed inventory against the original
`release.yml@refs/tags/v0.4.8` certificate and source SHA. No automatic fallback is
provided, and neither immutable release is changed.
