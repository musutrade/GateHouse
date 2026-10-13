# GH-230 local integration and clean-container acceptance

> Storage update (2026-10-13): historical raw logs/bundles are available through
> the [verified archive guide](archive.md). Dates, digests and outcomes below are unchanged.

P6.1–P6.2 and P7.1–P7.3 have local evidence below. Controller review and required CI
remain pending; this is not a release, baseline acceptance, or Arc-Admin certification.
The earlier [blocker](blocker.md), [Core continuation](core-0.4.0-continuation.md),
[configured continuation](configured-core-continuation.md) and
[malformed continuation](malformed-core-continuation.md) are historical records,
superseded for current status by this document. Their original failures and bytes
remain retained. No external workspace captures were used.

Predecessor #229 was accepted/closed and PR #235 merged as
`e24aeb351168610c00c14032a473fad61659ec2b`. This branch safely integrated main
`ee544690662645806cda7dd4cd2c9192566f7929`, preserving retry edits. Official Core
v0.4.0 is the distributed Linux binary, SHA-256
`8e3df8303ca8f650d4ef768b29cfefb60ca115122a47b116245cc19e4649bbdd`, from protected
release run 34586249753. Verification metadata and signatures are retained.

## Implementation and trust

The installed entry accepts Core's existing generic project collector envelope.
The host signs `collect --binding ABSOLUTE_PATH --binding-sha256 SHA256` through
existing configuration/trusted requests. The binding pins the project, invocation,
configuration/context, source/owner claims, independent capture anchor and delivery
manifest/matrix. Core authenticates requests, consumes nonces and owns policy.
The collector checks exact installed bytes, observed Core/tool/ABI identities and
receipt bytes before a complete native retained-binary re-export. Projection maps
validated owners to exact metrics and opaque raw artifacts. Missing owners cannot
leave partial usable evidence. No Rust-specific Core dispatch or fallback was added.

The binding schema is
[here](../../../tools/quality/schema/rust-project-collector-binding.schema.json);
[configuration fixture](../../../tools/quality/tests/rust_collector_config_fixture.py)
and [phased harness](../../../tools/quality/tests/rust_collector_clean_host.py)
show the concrete invocation. Distribution signatures only authenticate package
bytes; they do not authenticate captures. Synthetic candidate eligibility and a
disposable distribution key exercise installation without authorizing publication.
The generic request signer is separately provisioned by the test host.

## Frozen fixture combination

The exact [fixture manifest](https://github.com/musutrade/Harness-Gate/blob/d9a932b23dcfea709175285b594ca200450a9971/docs/quality/gh-230/fixture-manifest.json) and [matrix](https://github.com/musutrade/Harness-Gate/blob/d9a932b23dcfea709175285b594ca200450a9971/docs/quality/gh-230/fixture-matrix.json)
freeze the locally successful candidate tuple, including all payload/tool hashes,
protocols, configuration, projection/classifier identities, series and license
inventory. The matrix's receipt kind describes the native/Core evidence submitted
for review; it is not proof that controller review already occurred. The shipping
matrix stays empty until the separately authorized release process consumes approved
receipts. No additional target, ABI minimum, policy or series equivalence is inferred.

| Dimension | Observed value |
| --- | --- |
| Collector candidate | `0.0.0-rc.230`; main base `ee54469` plus this change's payload hashes/source snapshots, not a published collector |
| Core | Official `0.4.0`, full source/SHA above |
| Container | Fresh pinned scratch image `sha256:e9c7b0cca502ceb3fd8fbc377ab61765ef646ef68a178ff0046007db14c447f6` |
| Host ABI | Linux x86_64 GNU; glibc 2.43; kernel `7.0.0-31-generic`; exact library fingerprint in manifest |
| Runtime | Private Python 3.14.4; rustc 1.97.1 (`8bab26f4f68e0e26f0bb7960be334d5b520ea452`); LLVM 22.1.6; inventory `rustc-mir-block-inventory/3` |
| Native scope | Single-file Rust fixture; cyclomatic complexity, function/line/region coverage, CRAP |
| Normalized series | `4e82ec1a4fc920e070dc14bc3a33b1592e7443fe1c517aa580c7d3fc62b178b9` |

The operator queue starts each phase with networking disabled, uid 1000, no
capabilities, read-only installed runtime and a separate writable evidence mount.
It supplies only the pinned Core and independent strace observer in addition to
the declared runtime/test inputs. No source checkout, ambient Python, Docker socket,
network download or global toolchain mutation is needed in the container. The
host installer itself runs outside that container with independently pinned host
OpenSSL/library trust. This certifies clean-container execution on the observed
supported host, not installation on a newly provisioned VM or any other host.
The provisioned official rustc-dev archive is a build input, not acceptance evidence.

## Native execution and Core decisions

The archive's `final-acceptance-v3/run-phase.py` records the exact wrapper command,
`strace -f -s 4096 -e trace=execve` and private Python invocation for each phase.
Every phase has its full command, exit, wall time, container receipt and raw trace.
`seed` creates fresh base/head fixture captures and two matching complete native
re-exports; `prepare` compiles actual generic configuration; `collect` invokes the
installed collector using an authenticated request; `evaluate` runs official Core.
Seed ran with the preceding candidate whose native/projection bytes match the final
candidate; installed collect/evaluate/cost run with the final installed payload.

The valid measurement reaches Core and produces aggregate **fail**, including CRAP
56/1; it is not a measurement error or collector verdict. Synthetic capability
negatives over fresh native facts preserve actual Core decisions: `unsupported`
and `not_configured` block; `not_collected` skips; `not_applicable` remains so;
`measurement_error` remains an error. These negatives do not supply native positive
measurement. Existing requiredness, thresholds, lineage, debt/ratchet, CRAP and
coverage rules and frozen Python C/D policy oracle semantics remain unchanged.

Stale context rejects before collection. Positive collection launches once; replay
rejects at the fresh artifact-root guard. The malformed zero-exit producer launches
once, is rejected by Core, and its nonce replay cannot launch again. Wrong Core,
unknown combination, wrong fixed tools, relocated capture tools, structurally
missing owner and absent native owner reject with no projected usable evidence.
Actual physical runtime relocation preserves payload bytes but does not establish
capture-path or measurement-series equivalence. An absent native owner is rejected
only after the full native re-export, proving the native owner check is exercised.

Independent successful `execve` counts are retained in
[producer-counts.json](https://github.com/musutrade/Harness-Gate/blob/d9a932b23dcfea709175285b594ca200450a9971/docs/quality/gh-230/producer-counts.json), including every argv and separate failed
PATH lookup. Version probes are distinct from sampling; failed linker PATH probes
are not fallback collectors. Counts below are complete phase totals.

| Phase | Compiler | Fixture sample | profdata merge | LLVM export | Collector | Core collect/evaluate |
| --- | ---: | ---: | ---: | ---: | ---: | --- |
| Seed | 2 | 2 | 6 | 6 | 5 | 0 / 6 |
| Prepare | 0 | 0 | 1 | 1 | 1 | 0 / 0 (one config compile) |
| Installed collect, stale and replay | 0 | 0 | 1 | 1 | 1 | 3 / 0 |
| Installed evaluation (including baseline re-export) | 0 | 0 | 1 | 1 | 1 | 0 / 1 |
| Six direct delivery/owner negatives | 0 | 0 | 1 | 1 | 6 | 0 / 0 |
| Physical relocation | 0 | 0 | 0 | 0 | 1 | 0 / 0 |
| Malformed and replay | 0 | 0 | 0 | 0 | 0 (one malformed script) | 2 / 0 |
| Measured capture and two re-exports | 1 | 1 | 3 | 3 | 0 | 0 / 0 |

## Cost and durable evidence

[Setup measurements](https://github.com/musutrade/Harness-Gate/blob/d9a932b23dcfea709175285b594ca200450a9971/docs/quality/gh-230/setup-cost-measured.json) record the real package SHA
`24ba89a7bdf43cc84237221461ef8df1875a0ae19e4530c05a1d97c27adabdc2`, size
1,623,490,560 bytes; all release assets total 1,624,188,998 bytes. Inventory/build
assembly took 25.12/20.15 seconds. A fresh install directory took 38.4017 seconds,
then warm selection 16.7161 seconds. At 20 ms sampling, peak install allocated disk
was 3,249,242,112 bytes, logical 3,246,839,564 bytes (1,758 observations). Installed
logical bytes were 3,246,779,504, including the retained distribution archive.
The first successful unmonitored install took 69.1780/17.9869 seconds; both records
are retained. Cold means a fresh directory; OS caches were not flushed. Install
peak excludes the immutable input release and is sampled, not an exact maximum.

The separate clean-container cost run records capture 1.6104 seconds, first complete
re-export 1.1498 seconds and warm complete re-export 1.0016 seconds. At 10 ms
sampling, peak allocated capture disk was respectively 11,694,080 / 11,722,752 /
11,751,424 bytes. Hardlinks count once; immutable runtime is excluded. Both reports
match exactly and bind anchor
`de806dd717e0eb0134dc289654d9e73d2bad0603c2c957309dbefc3a4d834fbc`.
Phase command wall time includes wrapper/observer overhead; installed generic
collect was 27.17 seconds and evaluation including baseline re-export 9.08 seconds.
These are observations, not performance guarantees.

[clean-host-evidence.tar.gz](https://github.com/musutrade/Harness-Gate/blob/d9a932b23dcfea709175285b594ca200450a9971/docs/quality/gh-230/clean-host-evidence.tar.gz.parts/manifest.json) retains original native
executables, profiles, inventories, source, anchors, seals, exports and replay
outputs; generic requests/decisions; raw traces; package metadata, public test trust,
license inventory, commands, failures and source snapshots. Its
[archive receipt](https://github.com/musutrade/Harness-Gate/blob/d9a932b23dcfea709175285b594ca200450a9971/docs/quality/gh-230/clean-host-archive.json) and [per-file inventory](https://github.com/musutrade/Harness-Gate/blob/d9a932b23dcfea709175285b594ca200450a9971/docs/quality/gh-230/clean-host-inventory.json)
verify all 812 retained files after compression. The 1.62 GB package and duplicate
relocated runtime stay workspace-local; exact inventories and hashes are retained.
Disposable private signers, operator secrets and Python bytecode are excluded.
This is retained-binary native re-export evidence, not just JSON report replay.

## Genuine failures and corrections

All prior GH-230 records remain unchanged. The final archive also preserves:

- Seed import failure: test harness lacked its quality-module path; corrected test
  input paths before successful seed.
- Candidate v2 tool versions contained newlines rejected by the strict evidence
  JSON domain; v3 deterministically joins version lines and keeps exact byte hashes.
- Initial installer trust named a symlink loader; independently pinned trust now
  names the resolved regular library, retaining its original hash.
- First collect replay assertion expected a nonce error, but Core rejected the
  reused artifact root earlier. The harness now asserts that actual guard and
  snapshots the positive output before replay.
- Initial baseline context used the wrong base commit and Core correctly returned
  measurement_error. Correct fixture lineage binds the head's expected base; the
  new native baseline re-export yields the actual policy fail.
- A redundant standalone stale probe hit the fresh-root guard first. Its failed
  assertion/log/trace remain retained; the final harness removes that redundant
  phase because collect already verifies stale-context rejection before launch.

Local validation failures and corrected results are in [final-validation.md](final-validation.md).
No acceptance task claims hosted CI passed. P8 release approval/publication and
GH-215 owner coordination remain separate; no Arc-Admin writes or baseline changes
occurred.


### Evidence transport

Shell Git cannot connect to GitHub under this sandbox and the authenticated API
transport timed out on a 12 MB blob. Archives above 2.88 MB are therefore retained
as numbered byte parts. Each linked parts manifest records the original archive
SHA-256/size, every part hash and the concatenation command. Concatenated bytes
were verified against every original archive; existing per-file inventories remain
valid. Original unsplit archives remain in the workspace. This transport change
neither regenerates native captures nor changes their bytes.
