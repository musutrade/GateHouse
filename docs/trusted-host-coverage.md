# Trusted host migration inventory

This inventory accompanies [the trusted host](trusted-host.md). It does not grant
local coverage acceptance. Every row still belongs to the existing Actions job;
no current trigger, required dependency, threshold or routing rule changes here.

Source: [CI workflow](../.github/workflows/ci.yml) at repository revision
`19fc936e2d5d1fa54768c20bf44d3187b7c841dc`, workflow SHA-256
`69a811afc4ca0db934813945110fc0e28cc2411ffaab4f64762fa3ae068c24a8`.
Regenerate the comparison against the final deployed revision before acceptance.
The workflow itself contains the exact environment, pinned actions, command
arguments, fixtures and artifact retention requirements; this table identifies
which complete unit must be reproduced, rather than replacing its specification.

## Required execution cells

PR and main pushes, plus daily/manual full runs, use the existing impact routing.
Native build and native CLI contract jobs run outside PR events. Native tests do
run on applicable PRs. A routing-based skip is not a migrated native PASS. Local
full mode must cover every applicable cell before publishing its own success.

| Current job / environment | Configuration and complete workload | Evidence to accept | Local owner / acceptance |
| --- | --- | --- | --- |
| `test` / Linux | Stable Rust; locked default test/dev; pinned `arc-admin-replay` and `regression-corpus`; nextest; both findings-feedback and report-artifact Node test suites; synthetic HTML provenance/transport; workflow acceptance; both generated schemas; real locked GateProbes Rust/Python onboarding at `53c01505bed2ff61dfad1d106abbec2dcd961d05` | All test exits, synthetic HTML provenance and transport contract, workflow-acceptance directory, unchanged schemas, onboarding report/logs | Unassigned; pending |
| `test-cross-platform` / macOS | Native stable Rust, locked default tests, same two fixture groups; `cargo nextest run ... --no-fail-fast` | Native test results with actual platform/tool identity | Unassigned; pending |
| `test-cross-platform` / Windows | Native stable Rust, locked default tests, same two fixture groups; `cargo nextest run ... --no-fail-fast` | Native test results with actual platform/tool identity | Unassigned; pending |
| `security-audit` / Linux | Stable Rust; approved cargo-audit; `cargo audit --deny warnings` in `tools/harness-gate` | Audit result and advisory database identity/freshness at execution time | Unassigned; pending |
| `fmt` / Linux | Stable rustfmt; `cargo fmt --manifest-path tools/harness-gate/Cargo.toml -- --check` | Successful format check | Unassigned; pending |
| `clippy` / Linux | Stable Clippy; locked all-targets/all-features dev/test; `-D warnings` | Successful complete Clippy output | Unassigned; pending |
| `build` / Linux | Locked default release profile, existing LTO/codegen policy | Release build result and informational binary digest; binary is not new quality evidence | Unassigned; pending |
| `build-cross-platform` / macOS | Native locked release; existing `LTO=false`, `CODEGEN_UNITS=16` overrides | Native compile result and actual profile identity | Unassigned; pending |
| `build-cross-platform` / Windows | Native locked release; existing `LTO=false`, `CODEGEN_UNITS=16` overrides | Native compile result and actual profile identity | Unassigned; pending |
| `quality-fast` / Linux | Approved nextest/llvm-cov/llvm-tools; isolated instrumented profile; full pinned data; `ci_quality.py collect --stages legacy,production` with actual base/head/run | Both stage publications, tool identity, existing quality artifacts excluding build/snapshots | Unassigned; pending |
| `quality-risk` / Linux | Same instrumented tools/data; `collect --stages risk`; existing `CI_PARALLEL_RISK=1` semantics | Risk publication, complete existing CRAP/critical risk inventory and tool identity | Unassigned; pending |
| `quality-matrix` / Linux | Same instrumented tools/data; `collect --stages matrix` | Matrix publication and tool identity | Unassigned; pending |
| `quality-coverage` / Linux | Exact fast/risk/matrix inputs from this invocation; `merge`, `verify`, `ci_artifact.py seal` | Complete Core-accepted coverage/critical-path results, immutable manifest and all retained declared artifacts | Unassigned; pending |
| `quality-contracts` / Linux | Stable Rust; full pinned data; default dev; `contracts.py --output ...` | Existing CLI snapshot contract result and snapshots | Unassigned; pending |
| `quality-contracts-cross-platform` / macOS | Native stable Rust/default dev; full data; `contracts.py --structured` | Native structured contract result and snapshots | Unassigned; pending |
| `quality-contracts-cross-platform` / Windows | Native stable Rust/default dev; full data; `contracts.py --structured` | Native structured contract result and snapshots | Unassigned; pending |
| `docs-consistency` / Linux | Full mode installs Rust and executes `docs_consistency.py`; current docs-only pure route remains defined in workflow | Documentation/examples/schema result; pure mode cannot stand in for full mode | Unassigned; pending |
| `release-contracts` / Linux | Full pinned data; `python3 -m unittest discover -s tools/release/tests -v`; `bash tools/release/tests/test_install.sh` | Both complete governance and install-contract results | Unassigned; pending |
| `quality-scripts` / Linux | Full pinned data and Rust coverage tools; physical source-cleanup check; `run_core.py`; compile quality scripts | Complete Core-owned test discovery, cleanup result, all script tests and compilation results | Unassigned; pending |

## Routing, publication and advisory cells

| Current job | Role | Migration obligation |
| --- | --- | --- |
| `classify` / Linux | Repository-size gate, reviewed migration baseline, path classification; PR equivalence receipt and exact push-reuse proof | Preserve repository-size enforcement and applicable coverage decisions. Bind base/head/tree/merged input in the trusted invocation. Existing Actions reuse receipts do not authorize a host PASS. |
| `quality-required` / Linux | Existing stable required aggregate evaluates all declared dependencies and event/routing state | Retain unchanged during transition. Replace only after every applicable execution cell and live trust acceptance passes, using the actual Checks App integration ID. Do not introduce an Actions verifier as the replacement authority. |
| `quality-generic-shadow` / Linux | Advisory projection/comparison of this run's sealed retained Rust evidence | Retain advisory status and evidence validation; do not promote it to authority or use it to replace required measurements. |

Other workflows retain their existing release, scheduled or advisory ownership.
Inventory any applicable consumer-specific workflows before deploying there.

## Gaps in the initial Linux executor

The initial offline serial sandbox does not supply macOS/Windows execution,
GitHub action transports, an advisory-database refresh service, reviewed network
acquisition for pinned plugins, or an accepted Git-history-dependent quality
workload. The opt-in worker Git view described below supplies the read-only
history prerequisite; its protected deployment was accepted in the limited
`659803a` isolation pilot. Complete script workloads still need acceptance.
Several existing quality scripts need that history. Schema generation has the
separate-output candidate adapter below; onboarding still needs writable working
copies while the trusted candidate remains immutable. These require reviewed step/materialization adapters and exact
evidence contracts before assigning a local owner. Running a smaller Linux suite
does not close those rows.

### Opt-in worker Git history

A reviewed step may set `git_history = true`. The root launcher prepares a
separate bare Git repository beneath that invocation's private control directory,
with exact candidate/base refs and an index matching the frozen source. It uses
an empty template and fresh configuration; controller remotes, proxy settings,
credentials, hooks and object alternates are not copied. Root checks clean source
bytes and records the complete file inventory, digests and run/head/base/tree
binding before reuse. A changed or interrupted view stays blocked and retained.
The view has a 256-MiB total input budget and a 128-MiB individual-file read limit.

Only that view is bound read-only to `/work/history`, with a synthetic read-only
`.git` pointer over the original source metadata file. Controller Git metadata
and private directories remain inaccessible. Native Git discovery selects the
view in the source work tree; nested test repositories keep their own Git
identity. No global `GIT_DIR` or `GIT_WORK_TREE` retargets fixture commands. Git
disables optional index writes and ignores system/global configuration.
The worker receives exact safe-directory entries for `/work/source` and
`/work/history`, not a wildcard. A step cannot override these Git inputs through
its environment. Source files remain immutable and writable outputs stay separate.

This supports source reads, base/head differences, clean-status checks and source
archives. It does not grant Git writes or supply writable schema/onboarding
copies, dependencies, native execution or complete quality coverage. The installed
`659803a` isolation pilot opted into the adapter and passed its actual worker
mount/UID/history probe. Each migrated quality workload still requires acceptance;
table ownership above is unchanged.

### Candidate schema measurement adapter

[check-schemas.py](../tools/trusted_gate/deploy/check-schemas.py) runs the existing
Core's `schema export` and `schema export --quality` with an explicit, fresh
`--project-root` outside the frozen source. Core already supports that output
root; a writable candidate copy is unnecessary for these two pure exports. The
adapter checks both generated files byte for byte against the committed schemas.
It requires the exact candidate head, clean tracked bytes, matching Core crate
tree and reviewed binary digest, and rechecks those inputs after both commands.
The protected caller must provide those pins from its reviewed build/configuration;
arguments supplied by an Agent are not build provenance or acceptance.

Stale output, overlapping source/output, links, changed input, command failure,
timeout or missing output cannot produce a match. The result records exact
commands and both file digests; a mismatch/error exits nonzero. Final gate
requiredness and quality decisions remain in Core. Engineering-policy semantics
are unchanged.

An operator-domain measurement using the matching non-incremental release Core
generated both schemas from `659803a` and found identical bytes (34,227 and 11,928
bytes). The Cargo build output had two hard links, so it was copied to a separate,
byte-identical single-link staging file; the input guard was retained. This is a
measurement of the real Core, not a protected worker deployment acceptance.
The adapter is not installed or selected by the pilot policy, is not included in
the fixed fa18 upgrade, and does not close the complete Linux Test row. The other
tests, onboarding, fixture/toolchain preparation and native cells remain pending.

### Candidate disposable Linux workspace

[prepare-linux-workspace.py](../tools/trusted_gate/deploy/prepare-linux-workspace.py)
is an unprivileged input-preparation adapter. It accepts the frozen source and
the sanitized history view, exact candidate/base identities, a reviewed manifest
digest and an already populated archive cache. A root invocation is rejected.
The protected caller supplies the pins and mounts; Agent-selected arguments are
not protected configuration or deployment acceptance.

It creates a fresh Git working copy outside those inputs, with an empty template,
no remotes/alternates, disabled hooks and no forwarded Agent Git environment.
Only the fixed candidate/base objects come from the sanitized local history.
Before writing, it checks source modes/count/size and the restoration budget.
It then reuses the existing project-owned `tools/maintenance/test_data.py` with
`--offline`, restoring every manifest group in the disposable copy. It does not
replace the existing archive validator or acquire missing archives implicitly.
The adapter independently hashes every restored member and every tracked file
against the original Git blob/mode inventory. A worker-writable Git index and
`skip-worktree` flags cannot hide changed source bytes. Core retains final quality
authority; engineering-policy semantics are unchanged.

Missing/corrupt needed cached data, changed pins/input, unsafe history configuration,
linked paths, oversized restoration or an existing/partial workspace blocks
preparation. Failure records remain outside the copy; no cleanup, resume or old
PASS reuse occurs. A `prepared` record is only an input inventory. A future
complete-job adapter must execute every applicable command, verify its complete
outputs and revalidate the tracked blob/mode inventory after the job.

The operator-domain input experiment at `f555690` restored and verified all three
existing manifest groups: 1,480 files, 49,853,339 unpacked bytes, from 36,845,164
archive bytes. Its initial missing `quality-history` cache entry remained an
explicit offline failure. The exact pinned release asset was then separately
downloaded and checked before repeating offline restoration; the worker flow has
no download fallback. Both Node integration suites and locked offline Cargo
`metadata --no-deps` also completed in that disposable operator workspace. These
are readiness measurements, not full Rust tests, onboarding, protected worker
execution or accepted Linux coverage. The installed `659803a` pilot, package
inventory, policy, required rules and workflow triggers are unchanged. This
candidate adapter still needs its own protected deployment review.

### Candidate Linux Test workload

[run-linux-test.py](../tools/trusted_gate/deploy/run-linux-test.py) prepares a
fresh disposable workspace and executes the existing Linux Test job's project
workload: the pinned nextest version check and complete locked nextest run, both
Node integration suites, actual Rust-generated HTML provenance and display-file
transport staging, configured workflow acceptance, both Cargo schema exports and
their Git comparison, and real locked Rust/Python onboarding from GateProbes
`53c01505bed2ff61dfad1d106abbec2dcd961d05`. Onboarding's 29-step receipt must bind
the actual newly built candidate Core. Cargo's legitimately hard-linked generated
executable is copied to a byte-identical private snapshot for onboarding; both
copies are checked afterward. Immutable-input hardlink guards stay unchanged.

The command plan refuses a changed CI workflow until parity is reviewed. The
onboarding input's complete file/size/content/executable-mode inventory is pinned
in the reviewed adapter alongside that exact commit; a directory label or an
Agent-supplied SHA cannot substitute another snapshot. A protected caller must
stage this snapshot from the pinned Git objects and mount it read-only, provide
the approved offline toolchains/dependencies and persistent Cargo target, pin
the adapter/helpers and actual tools in host policy, and retain the complete
invocation outputs. This candidate does not install or configure those inputs.

The worker receives a fresh home/temp and a narrow environment. Credentials,
Agent Git configuration and non-default compiler/profile overrides are not
forwarded. Every required command must finish successfully within the existing
20-minute job budget; failure or timeout blocks without retry. Zero tool exits
with missing HTML or onboarding output cannot complete the workload. After all
commands, the adapter rechecks both the frozen input and working copy against
the immutable Git blob/mode inventory, as well as restored regression data and
the pinned onboarding snapshot. A mutable index cannot hide source changes.
The existing Core remains the final quality authority; engineering-policy
semantics are unchanged.

The adapter invokes only the existing artifact preparation code to stage the
two validated display files; it calls no GitHub upload action/API. Existing CI
artifact retention must be replaced by the protected host's accepted evidence
publication before this cell can migrate. The numeric receipt identity exercises
the existing workflow contract locally and is explicitly not an Actions run.
Select the named receipts and logs for publication; onboarding's disposable
signing-key files and other scratch contents remain private and are not export
artifacts. Systemd still owns final worker-cgroup termination and resource limits.

The candidate host launcher accepts an administrator-pinned `capture_files` list
for each step. Paths are explicit and relative to that step's fresh scratch root;
there is no glob or scratch-directory export. After `systemd-run --wait` returns,
the launcher independently checks the exact worker unit and, when present, its
empty cgroup before reading anything. Missing, linked, multiply linked, special,
wrong-owner, oversized or concurrently changed files block the step. Capture is
bounded to 128 files, 4 MiB per file and 8 MiB total. Existing steps without the
selection retain their previous output behavior and also receive the stopped-unit
check before cache maintenance.

The launcher writes one JSON log entry with run/head/step, the actual worker exit
code, selected relative names, sizes, SHA-256 and base64 of every selected file's
complete bytes. This is diagnostic output in the existing Core step log. Core's
unchanged invocation, artifact registry and manifest bind that log, including the
embedded bytes; selected files are not invented individual Core declarations.
The entry does not authorize PASS. A nonzero worker exit remains nonzero, and any
capture failure or Core output-limit failure remains blocking. The unchanged
16 MiB Core stdout limit also covers other worker stdout; logs are not truncated
to manufacture success. No separate quality decision or credential format is added.

For Linux Test, the future reviewed deployment must explicitly select the
adapter result/preparation JSON, both HTML display files, workflow acceptance
receipt/summary, onboarding report, the ten command stdout/stderr pairs and all
29 onboarding stdout/stderr pairs. Disposable onboarding keys, plugin fixture
workspaces, generated executables and general scratch contents are excluded.
This capture candidate is not installed or enabled in the accepted `659803a`
pilot, and protected worker/cgroup acceptance is still pending.

An operator-domain transport observation at capture code `e67b037` selected 85
files from the completed `e3e9fa3` Linux Test run (2,951,405 original bytes).
The existing matching release Core sealed a 3,946,710-byte step log in its
canonical invocation, manifest and registry. Every selected file decoded exactly
to its original bytes and SHA-256; the disposable onboarding key was excluded.
This fixture tests retention transport, not new candidate quality or real
systemd/identity isolation. Failed fixture setup/configuration attempts were
retained separately; no production result was retried or changed.

An operator-domain run at `d3f0cbb` completed all 724 Rust tests with zero skipped,
both Node suites, real HTML validation, workflow acceptance, matching schemas
and all 29 pinned onboarding steps. The committed adapter at `e3e9fa3` then
completed its own fresh exact-candidate run: all ten command stages succeeded,
724 Rust tests passed with none skipped, all 29 onboarding steps completed,
HTML transport staged exactly its two validated files, and post-job verification
matched 1,620 tracked files plus all 1,480 restored inputs. This remains an
operator-domain measurement. Protected worker deployment and resource acceptance
are still pending. Neither observation changes the table's unassigned owners,
enables production, removes native requirements or replaces the remaining Linux
quality/build/audit/contracts jobs.

`deploy/stage-linux-inputs.py` is a separate unprivileged preparation candidate
for the offline runtime. It copies the full raw Rust sysroot, only Cargo's
`registry` subtree (not Cargo home/configuration/credentials), explicit nextest
and Node executables, the fixed onboarding source and only the test-data
manifest's SHA-256-addressed archives. Sources, output and parents cannot overlap
or contain symbolic links. Source tree inventories are checked before/after
copy; individual streaming reads reject hardlinks, special files, privileged
modes and mutation. Existing or partial output is retained and cannot be reused.
The helper performs no network operation or tool execution.

The separate input budget is 512 MiB per file, 6 GiB combined payload, 150,000
files/directories and 64 MiB inventory metadata. This accommodates large Rust
shared libraries without increasing the controller's configuration or evidence
limits. The snapshot uses readonly files/directories and records complete names,
bytes, SHA-256, executable modes and exact onboarding/test-data pins. Verification
rejects missing/extra/changed files, directories and permissions. Its optional
trusted verification additionally requires root execution, root ownership and
protected parents; an operator-owned snapshot is never accepted as that proof.
This is a runtime input inventory, not a quality result or credential. Ownership
verification alone does not establish upstream provenance. In particular, this
helper's operator-owned Cargo registry and Rust installation must not enter the
trusted execution domain through a chown/copy. The eventual administrator bootstrap
must independently acquire approved toolchain inputs and registry metadata, check
crate archives against the frozen source's Cargo.lock checksums, and reconstruct
or compare extracted crate sources against those verified archives. Only matching
independently verified inputs may be imported; this candidate does not implement
that trusted bootstrap and does not accept an Agent cache as authoritative input.

The staging helper at `b092937` prepared an operator snapshot of 116,914 files,
3,888,114,185 logical bytes, with a 19,597,186-byte inventory in 513.304 seconds.
Its inventory SHA-256 is
`47f1d63ac6899d76af5068e2270154df28ecc90c4781f5711cc7a5951c85bcb0`.
Copied Rust 1.99.1, Cargo 1.99.1, nextest 0.9.143 and Node 24.18.0 started
successfully with the original homes hidden and network limited to loopback in
an ordinary user namespace. Mount verification rejected chmod with EROFS.
These are operator preparation/runtime observations, not trusted bootstrap proof.

The first full namespace attempt retained a real cache-contract failure: the
observation parent inherited mode 0775. Core correctly refused that ancestor;
tightening only this private observation directory to 0700 made the exact
contract pass, without changing Core or its tests. The next attempt passed all
724 Rust tests with zero skipped, both Node suites, HTML/workflow acceptance and
schema checks, but blocked on onboarding: its upgrade fixture copied readonly
input modes and then could not edit its own copied catalog. Both failed attempts
remain separate evidence and are not accepted complete executions.

The adapter now makes a private writable onboarding source copy, verifies its
complete content/executable-mode inventory against the same fixed pin, and keeps
the original readonly input unchanged. The existing upstream acceptance test,
including its corruption/upgrade/rollback cases, remains unchanged. A focused
namespace recheck at `d8b3aa8` passed all 29 actual onboarding steps and verified
both source copies' pins. This does not substitute partial runs for a full Gate.

The corrected adapter at `d8b3aa8` then completed its own full namespace run in
256.052 seconds: all ten stages succeeded, all 724 Rust tests passed with zero
skipped, all 29 onboarding steps passed, and post-validation matched 1,623 tracked
files plus all 1,480 restored inputs. It used the readonly copied runtime and
registry with original homes hidden and loopback-only networking. Only compiled
target artifacts were reused from the diagnosed earlier attempts; all commands,
reports and output workspace were fresh. This is not a cold-cache success or
root/systemd/UID995 acceptance. It grants no coverage migration or App authority.
A separate read-only audit matched all 205 cached crate archives selected by the
frozen Cargo.lock to their locked SHA-256 values; it does not yet verify extracted
sources, registry metadata or compiler distribution provenance.

`deploy/verify-locked-crates.py` separately compares the frozen Cargo.lock,
SHA-256-verified `.crate` archives and Cargo's original extracted source trees.
Its caller supplies a separately trusted lock digest and immutable inputs. Only
the selected crates.io registry packages are checked; local packages are listed
as unchecked, and unrelated cache packages are outside this comparison. Unknown
registry/Git sources, duplicate identities, changed archives, missing/extra files
or directories, byte/executable-mode changes, links, special files, privileged
modes and observed read-time mutation block the comparison. The helper reuses
the existing archive path validator, reads tar members without extraction, bounds
compressed/decoded payloads and metadata, and changes no input or installation.

Every selected source file, including hidden files, is compared against its
archive. Only Cargo's generated `.cargo-ok` extraction marker is excluded from
the file inventory, after strict version-1 JSON validation if present. A supplied
`.cargo-checksum.json` is not trusted to override archive content. A trial of
standard `cargo vendor --frozen --versioned-dirs` generated all 205 directories,
but omitted some archive `.gitignore` files; it therefore does not establish this
whole-source equivalence. No relaxed hidden-file exception was added.

An actual readonly, network-isolated operator namespace comparison matched all
205 locked registry packages: 8,720 files and 194,025,034 bytes in 11.041 seconds,
against Cargo.lock SHA-256
`a7cbc8d42763de46c79369f5d87a9efd430731038ff0633319dad850cfe4d461`.
Thirteen focused negative/positive tests and all 119 trusted-host tests passed.
This closes the selected archive/source comparison gap only. The helper does not
validate registry index metadata, local dependency sources or compiler/tool
distribution provenance, create a trusted bootstrap, import an Agent cache,
change the installer inventory, or authorize a quality result. Future protected
use must provide protected immutable inputs and independently trusted pins;
ordinary-user observations do not provide Root/systemd/UID995 acceptance.
Engineering-policy authority, thresholds, required coverage and ratchets remain
unchanged.

`deploy/linux-tools.lock.json` fixes eight Linux x86-64 tool archives: Rust
1.99.1 compiler, Cargo, native standard library, rustfmt, Clippy and LLVM tools;
nextest 0.9.143; and Node 24.18.0. The pins were independently obtained from the
official dated Rust channel manifest with its matching same-origin SHA-256,
nextest's official GitHub release asset digest, and Node's official SHASUMS256
file. Source metadata URLs and digests are recorded in the lock. This is HTTPS
metadata/checksum verification, not a signature or Sigstore attestation. The six
selected Rust component pins match the operator's stored manifest, whose full
serialized bytes differ from the independently acquired upstream manifest.

`deploy/acquire-linux-tools.py` is an unprivileged archive acquisition candidate.
It requires the separately reviewed lock SHA-256, exact versions/target/archive
inventory and fixed official URLs. HTTPS redirects are restricted to the original
host or GitHub's release-assets host. Downloads stream into private partial files
with checksum, declared/actual size, socket/deadline and combined-byte limits;
there is no automatic retry, extraction, tool execution or installation. Completed
archives become readonly. A fresh destination and acquisition record are required;
failure retains partial/completed files and a blocked record without serializing
exception text that could contain signed redirect or proxy URLs. Operator network
settings are never copied into a lock, report or deployment. Root execution is
rejected; a future protected bootstrap must separately review its environment.

Actual acquisition matched all eight official archive digests: 218,817,177 bytes
in 46.174 seconds, with tool-lock SHA-256
`6a1ee8a6fb5fa23c9fe9d8835d2c2f0ab7fcea2fa9e33493dd4b958432f4e1cf`.
Nine focused acquisition regressions and all 128 trusted-host tests passed. This
does not prove installed sysroot equivalence, platform ABI compatibility, full
workload parity or protected-worker acceptance. Before any future import, Root
must verify archive bytes against the protected reviewed pins before unpacking or
executing them; an existing operator sysroot is not promoted by chown/copy.
The existing installer/package inventory and accepted pilot remain unchanged.

After acquisition, an ordinary user namespace reconstructed all eight official
archives with the standard Rust component installers and exact Node/nextest
binary reads. The resulting tool directory contains 185 files, 1,020,204,078
bytes and no links; reconstruction took 23.247 seconds. At exact `4e4477b`, the
complete Linux Test adapter then passed all ten stages with a fresh empty
compiled target: 724 Rust tests, zero skipped, both Node suites, HTML/workflow
acceptance, schemas, 29 onboarding steps and post-validation of 1,628 tracked
files plus 1,480 restored inputs, in 322.014 seconds. Original homes and external
networking were hidden, with inputs mounted readonly. This is UID1000 namespace
evidence for Linux Test only; it does not supply Root/systemd/UID995 acceptance,
other matrix cells or trusted import of the operator's registry metadata.

`deploy/prepare-locked-directory.py` is a separate unprivileged preparation
candidate for Cargo's directory-source replacement. It consumes only the frozen
lock and selected `.crate` archives, never an existing extracted tree or index.
Archives are streamed into private copies, checked against locked SHA-256 values
and validated with the existing archive/source contract before standard-library
extraction. Original input changes after the verified copy cannot replace that
snapshot. The complete source inventory, including `.gitignore` and other hidden
files, is independently compared after extraction. Cargo checksum metadata is
derived from those archive facts, then checked independently alongside all source
bytes, executable flags and directories; supplied cache metadata is never trusted.

The output contains readonly source directories, copied archives, the frozen lock
and a canonical inventory. Verification derives the facts again from the locked
archives, rejecting forged checksum/inventory metadata, changed source, missing
or extra packages/files/directories, cache markers, links and writable inputs.
The combined archive/source payload is bounded to 2 GiB, with the existing
128-MiB file, 256-MiB package, 100,000-source-file and 4-MiB checksum metadata
limits, plus a 64-MiB inventory limit. Old/partial output is retained and refused;
failures record blocked preparation. Root execution, network access, Cargo/tool
execution, installation and quality authorization are outside this preparer.

The actual preparer rebuilt 205 packages and 8,720 archive source files totaling
194,025,034 bytes, plus 29,855,126 bytes of copied archives, in 24.462 seconds.
An independent offline Cargo metadata run against this prepared directory and an
initially empty Cargo home resolved all 205 registry packages plus two local
packages in 0.215 seconds. Cargo created only `registry/CACHEDIR.TAG`, with no
registry index. All 138 trusted-host tests passed, including ten new directory
preparation regressions. This proves preparation and dependency resolution only;
complete workload parity with this new source replacement remains pending.
Cargo's generated checksum file is not a malicious-change security boundary:
protected immutable inputs and independently trusted archive/lock pins are still
required before a future Root bootstrap may import or execute anything. The
engineering-policy authority, thresholds, coverage and ratchets are unchanged.

`deploy/prepare-official-tools.py` assembles the reviewed tool payload directly
from the eight pinned official archives. It copies archives into private output,
checks the separately reviewed tool-lock digest and every archive SHA-256, bounds
decoded bytes/member counts/metadata, and validates paths and member types. For
each Rust archive, the exact expected component and every manifest-declared file
must match the payload. Node and nextest contribute their exact executable files.
Selected links, special files, duplicate members, conflicting destinations and
missing/non-executable tools block preparation. Unselected Node package-manager
links are checked to stay inside their archive root and are never materialized.

The helper uses the standard tar reader for selected file streams; it does not
execute package installers, Cargo, build scripts or network requests. Output
retains the copied archives and reviewed lock alongside a readonly tool directory
and canonical inventory. Verification independently derives the complete expected
payload from the locked archives, then compares all bytes, executable flags and
directories; changing an inventory cannot conceal a changed executable. Mutated
inputs, stale/partial output, extra files, links and writable inputs block reuse.
The separate tool budgets remain 512 MiB per file, 2 GiB decoded per archive,
2 GiB combined compressed archives, 6 GiB payload, 150,000 members/files and
64 MiB inventory metadata. Controller/evidence budgets and Core policy are unchanged.

Actual assembly produced all 173 Rust component-manifest files plus Node and
nextest: 175 files, 1,020,148,204 payload bytes and 218,817,177 retained archive
bytes, in 166.726 seconds. The ten files generated by the standard Rust installers
are bookkeeping outside the archive component manifests; this private tool prefix
is not a Rustup-managed installation. The helper executes no installer code and
rejects Root execution. Its ordinary namespace probe started all six tools with
the expected identities, hid the original homes/network, and rejected chmod with
EROFS on the readonly mount. Twelve focused preparation regressions and all 150
trusted-host tests passed. Protected ownership/import, live resources and protected
workload acceptance remain separate obligations before any Root upgrade.

At executable code `e1839df`, a new ordinary namespace combined these 175 tools
with the 205 archive-derived directory sources. A readonly Cargo-home config
selected the directory replacement; the old registry and original homes were
hidden and networking was limited to loopback. From a fresh empty compiled target,
all ten Linux Test stages passed in 259.393 seconds: 724 Rust tests, zero skipped,
both Node suites, HTML/workflow acceptance, schema checks, 29 onboarding steps and
post-validation of 1,632 tracked files plus 1,480 restored inputs. Cargo created
only `registry/CACHEDIR.TAG`, without an index. Independent post-run verification
matched every tool and dependency byte/mode against the locked archives again.
The earlier preparation-only
observations are not substituted for this distinct full execution. These figures
establish ordinary Linux Test compatibility for the new inputs, not Root/systemd/
UID995 acceptance, performance improvement, full matrix migration or App authority.

The accepted `659803a` pilot remains on its existing 2 GiB volume. Neither this
preparation helper nor its ordinary-user observations install inputs or modify
that filesystem. A future reviewed upgrade must preserve its outstanding source,
journal and canonical publication before retirement, import independently checked
inputs into a separate bounded filesystem, select the full Linux Test adapter
and its 85 capture paths, and set measured resource budgets. Toolchain, scratch,
persistent cache and evidence remain separate; capacity availability alone is not
quota approval or protected-worker acceptance. No old installer/fa18 upgrader is
valid for this transition. All matrix owners remain unassigned until live checks.

### Candidate protected runtime bindings

The controller candidate accepts an optional reviewed `runtime` object with
`root`, `inventory` and `tool_lock_sha256`, plus an optional pinned `cargo_config`
file. Every path and parent must be Root-owned, free of links and outside writable
source, scratch, cache and submission domains. Mount operands reject whitespace,
relative traversal, colons and systemd escape/specifier characters. Raw registry
or Git input mounts cannot be combined with the readonly Cargo configuration.
Without these fields the pilot's existing input selection remains unchanged.

The runtime inventory must be pinned in `protected_files` and match the official
payload schema, exact canonical metadata, file/directory inventory and reviewed
tool-lock digest. `runtime.py` streams every actual payload file to compare its
size, SHA-256, readonly mode, single-link inode and identity before/after reading.
Unexpected or missing files, directories, links, special files, writable modes,
malformed rows and changed metadata block execution. Separate runtime limits are
512 MiB per file, 6 GiB total, 150,000 files/directories and 64 MiB metadata; the
ordinary trusted-file limit remains 128 MiB. Policy applicability rechecks the
payload before validation and publication/reconciliation; the worker launcher
also verifies it before selecting a cache or starting a command. A protected
inventory is a reviewed input pin, not archive provenance by itself: a future
Root importer must independently validate the official archives and payload
before establishing ownership and approving these pins.

The approved runtime mounts readonly at `/opt/gatehouse/toolchain`, using the
existing fixed worker PATH. The approved Cargo config mounts readonly at
`/work/output/cargo-home/config.toml`. PATH, loader, Cargo-home and Git overrides
remain forbidden. A step can explicitly opt into `run_arguments = true` to
replace only whole `@gatehouse:head@`, `@gatehouse:base@` and
`@gatehouse:receipt@` tokens. The receipt is the positive decimal value of the
canonical journal UUIDv4, never an Actions-run claim. An additional
`@gatehouse:test-data-sha256@` token reads only the fixed frozen
`test-data/manifest.json`. Unknown/embedded tokens, malformed UUIDs and mixed
journal/input heads block execution. Cache compatibility is selected from the
stable reviewed argument template and actual tool/dependency/configuration
inputs before per-run substitution; changing the head or UUID alone does not
discard compatible compiled artifacts. Every invocation still requires fresh
quality execution and evidence.

Fifteen new regressions bring the focused suite to 165 passing tests, including
stream mutation, payloads above 128 MiB, publication invalidation, mount guards
and cache/render ordering. At code `23e062c`, an ordinary UID1000 probe streamed
all 175 files / 1,020,148,204 bytes in 3.601 seconds and launched all six tools
at the fixed mount location with networking limited to loopback and original
homes hidden. Chmod on the runtime and Cargo config returned EROFS; the rendered
head/base/receipt/manifest arguments matched the journal fixture. This probe
replaces Root-ownership checks only for developer fixtures and provides no
Root/systemd/UID995 or full-workload acceptance. The release package explicitly
includes and pins the new runtime module. The
installed `659803a` pilot and its outstanding evidence remain untouched, and its
old fa18 upgrader is still unsuitable for this candidate. Engineering-policy
authority, thresholds, coverage requiredness and evidence budgets are unchanged.

### Candidate protected input import

`package-bootstrap-inputs.py` exports an exact committed helper tree and a
canonical manifest for separate administrator review. The closure has twenty-three files:
the importer, Core builder, preservation, volume, source-staging, release-contract, preflight, Linux-policy, input-sealing, host-configuration, Core-inspection, installation-barrier, controller-observation and upgrade-journal libraries, eight helper/schema resources under `tools/trusted_gate/deploy`
and the archive-path helper under `tools/maintenance`. Relative layout is retained.
This is a separate input-bootstrap package; the normal pilot package does not
silently install it. Packaging rejects Root execution and dirty source, and keeps
`administrator_ready` false. A reviewed privileged installer must establish the
trusted entry point and manifest pin before any administrator executes it.

`import-protected-inputs.py` requires Root and verifies the Root ownership and
permissions of every code path and parent, the administrator-reviewed manifest
digest, every helper digest and the complete file/directory inventory before
loading project helpers. Extra files/directories, injected bytecode, links,
hardlinks, missing files, writable code and malformed metadata block import.
The existing preparation helpers still reject Root at their own entry points;
this separate importer uses only their bounded archive-validation primitives.
It never runs package installers, Cargo, build scripts or network requests.

The source lock is read from the exact candidate Git object and compared with the
Root-protected working file, with exact head/base ancestry checks. Before running
Git, the importer checks the worktree/common Git stores for protected ownership,
ordinary file types, links and size/count bounds, and rejects external object
alternates and grafts. Hooks, fsmonitor and external protocols remain disabled.
Only lock-selected crate archives are copied, even when an operator cache has
additional packages. Official tools are selected by the reviewed tool lock.
Every selected archive is hashed into fresh private storage before parsing or
extraction; no operator sysroot, registry index, source directory or inventory is
promoted by changing ownership. The importer independently derives full tool and
dependency contents, preserves hidden source files, generates Cargo checksums and
compares the finished readonly trees against its retained archives again.

The destination must be fresh, disjoint from all inputs and under a private
Root-protected parent. A binding records candidate/base, helper revision/manifest,
tool lock and source-lock digests. Source/code pins are checked again before
completion. Failure retains a blocked control record and partial output; neither
can be silently reused. Entry-point paths retain strict operand validation, while
safe archive filenames are treated as data and may contain spaces, brackets,
at signs and percent signs. The original archive containment/link/type budgets
remain unchanged. Fifteen new focused regressions bring the suite to 180 tests.

At code `cb02c4e`, a full-data ordinary UID1000 fixture reconstructed all 175
tool files / 1,020,148,204 bytes and 205 locked dependency packages / 8,720 files /
194,025,034 bytes in 190.455 seconds. This fixture replaced the Root UID/ownership
checks; archive hashes, private copies, extraction, full byte/mode inventories,
Git-object comparisons and final source/code rechecks executed normally. It is
not a real Root import, a worker test or quality acceptance. An earlier failed
fixture is retained separately with its blocked record. The final candidate also
rejects double-leading-slash entry/mount paths; its exact helper authentication,
frozen-source lock binding and actual non-Root CLI denial were checked separately.
No new full Linux Test run is claimed by this input-import observation.

This candidate does not install a release, change the accepted `659803a` pilot,
build the trusted Core, start a privileged worker or authorize delivery. The
fixed 659 upgrade must still preserve and verify its complete source, journal and
canonical publication before revocation/retirement, then establish the bounded
storage, protected import, isolated Core build and actual UID995 execution.
Engineering-policy authority, requiredness, thresholds and evidence limits remain
unchanged.

### Candidate isolated Core bootstrap

The authenticated importer now offers a separate `build-core` action and includes
`build-protected-core.py` in its authenticated helper closure. Import completion
also seals the tool and dependency inventory digests in the binding/control
records. The builder requires these pins, matching retained source/head/base,
the reviewed tool lock and a complete import record. Before and after a build it
streams the actual tools, dependency sources/checksums and retained archives;
rewriting an inventory cannot conceal modified inputs.

Only the independently reviewed Core tree
`a6918d6a7ff7fadc04170ff54aaac0db771763db` is eligible. The builder verifies tree
and blob object identities and materializes a new readonly crate-only snapshot,
with no parent repository Cargo configuration. The actual snapshot contains 316
files / 2,748,319 bytes. Source content, readonly modes and complete inventory are
checked again before binary capture. An explicit host recipe pins the resolved
compiler, archiver, environment-cleaning program and management tools. OS-managed
hardlinks are permitted only by the separate host-file reader, with Root-owned
non-writable parents/inodes, stable file identity and reviewed byte hashes;
links in imported data, helper code and source snapshots remain forbidden.

The fixed command is `cargo build --release --locked --offline --bin harness-gate`
with a fresh target, four build jobs and release incremental compilation disabled.
It runs under UID995/GID981 with no privileged groups, capabilities, namespace
creation or network access beyond loopback. `/work` is a fresh Root-owned tmpfs;
source, tools, dependencies and Cargo config are readonly mounts, and only worker
scratch is writable. An explicit `env -i` clears inherited variables before Cargo;
the root-level Cargo configuration and operator homes/control paths are hidden.
Candidate limits are 1,200 seconds, 8 GiB memory and 1 GiB per generated file;
these are build-specific settings awaiting actual protected-worker measurement.

The builder holds the existing controller lease and supports retaining a caller's
already-held descriptor. This fixed migration requires the accepted 659 run and
frozen slot to remain outstanding and idle throughout bootstrap. The upgrade must
build and capture the replacement before revoking or retiring 659; a failed or
interrupted bootstrap does not release its old source. Previous Core, worker and
bootstrap units must be stopped before another build starts. Capture requires
the build unit inactive/collected, an empty cgroup when present and no UID995
processes. Timeout, nonzero exit, unknown stop state, source/tool/config drift or
unsafe generated files retain blocked evidence and export no binary. Only after
fresh checks does Root retain the fixed release binary with its source, host,
input, command, profile and digest record. Generated Cargo binary hardlinks are
accepted only at this capture boundary. Build completion grants no quality verdict.

This is candidate code and ordinary-user compatibility work. No Root service,
actual UID995 build, installer, accepted pilot, App or repository rule is changed.
The fixed 659 preservation/upgrade, bounded filesystem, actual process/resource
probes and full verification migration remain separate acceptance work.

At producer code `27ac413`, the ordinary full-data input fixture completed in
189.916 seconds and sealed both new inventory digests. At builder code `1f00a39`,
the crate-only readonly snapshot compiled successfully in an ordinary UID1000
namespace from a fresh target in 214.484 seconds, with release incremental disabled,
the official tool prefix, directory sources and a readonly Cargo config. Pre/post
checks of source, host recipe, tools, sources and retained archives took 10.538 /
10.602 seconds and passed. The resulting 17,106,920-byte ordinary artifact has
SHA-256 `1e7a342f45cbe5c82a915fc31f2a1c2441b01e8ad3e91bfc162da38916e66c95`;
its version is `harness-gate 0.5.0` and the existing pilot full/hook configuration
check passed. This is compilation compatibility, not Root/Systemd/UID995 capture
or full Linux Test acceptance. Wrapper RSS is not a cgroup memory measurement and
does not certify the candidate memory limit. The later fixed-659 slot/unit guards
are covered separately by lifecycle regressions; the measured compile recipe is
unchanged. Sixteen new bootstrap regressions bring the focused suite to 196 tests.

### Candidate fixed-659 preservation

The authenticated importer offers an administrator-only `preserve-pilot` action;
`preserve-pilot-state.py` was added as the twelfth helper resource. This candidate retains
private state without revoking a run, retiring source, changing installed code or
issuing a quality verdict. It requires the fixed accepted 659 head/run/release,
matching acceptance/configuration/marker, the sole idle pending publication slot,
unchanged policy and frozen source/bundle, stopped trusted units and no UID995
processes. It holds the existing controller lease and can retain a caller's
already-held descriptor. The output must be fresh, private and separate from the
old pilot volume and protected helper/configuration inputs.

The journal uses SQLite's backup API from a read transaction, including committed
WAL pages. Integrity, schema, complete ordered runs/events and their digests must
match the retained database; copying `journal.sqlite3` alone is insufficient. Only
the backup is normalized to DELETE journal mode, so reopening the retained
database requires no WAL/SHM sidecars; the original WAL remains untouched.
The candidate copies the complete source, repository and control trees, preserving
raw invocation reports, Git objects/worktree metadata, original bundle/freeze,
configuration, acceptance, marker, per-run records and validation logs. The live
journal sidecars and controller lock are excluded from that tree copy: the logical
database is retained separately and no active lock file becomes a restoration
instruction. File/directory modes are recorded, while retained files are private
and readonly. Source/configuration/database identity and all copied bytes are
checked again before completion.

Raw failed reports are retained even when the journal has no registered evidence.
Preservation does not reinterpret those reports as PASS. Partial output, unknown
schema/state, links, nonregular files, content drift or failed checks leave a
blocked record and never reuse the destination. Bounds are 64 MiB per journal
physical/logical snapshot, 50,000 rows per table, 256 MiB per raw file, 1 GiB total
raw-tree bytes, 30,000 tree entries and a 16 MiB final manifest. The future upgrader
must pin this private manifest outside the backup and call the retained-state
verifier before any destructive transition. Unknown or changed retained state
does not authorize retirement.

Seventeen new regressions bring the focused suite to 213 tests. Ordinary-user
fixtures replace Root ownership/authentication and unit probes; actual SQLite,
WAL transactions, linked Git worktrees, backup copies and digests are exercised.
An independent clone from retained Git objects recovers the fixture commit after
the original source and mirror become unavailable. The fixture uses
`--no-hardlinks` when preparing its mirror so it meets the unchanged protected Git
input rules. These are candidate retention/restore checks, not a live Root backup
or the complete fixed-659 upgrade acceptance. A reviewed bounded destination,
actual host backup/restore, replacement Core capture before revoke/retire, guarded
installation/recovery and full verification acceptance remain pending. Existing
659 files, stages, volume, source, journal and services remain untouched.

### Candidate separate Linux staging storage

`prepare-linux-volume.py` is the thirteenth authenticated helper resource; the
administrator-only `prepare-volume` entry prepares separate staging storage.
It does not resize, replace or remount the accepted pilot volume. The fixed
candidate is a 16 GiB image, an independent private registration and a separate
mountpoint. The controller lease, accepted 659 outstanding slot, stopped trusted
units and idle UID995 are required throughout. A pinned Root-protected recipe
identifies the seven actual resolved management programs, including `mke2fs` and
`e2fsck`; imported code/data link restrictions remain unchanged.

A durable private intent precedes exclusive image creation. The registered image
device/inode, size, UUID, helper/recipe pins and phase must match on every retry.
Unknown images, occupied targets, unregistered mounts, ambiguous signatures or
loop associations are retained for reconciliation. Allocation uses `fallocate`
with 2 GiB host free-space headroom. Physical blocks must cover the full image
before and after formatting and at mount admission. Formatting explicitly selects
ext4 with `nodiscard` and disables lazy metadata/journal initialization. A known
interrupted format with the expected UUID requires a successful readonly
`e2fsck -f -n`; it is never blindly reformatted. An image created before its inode
was sealed is not automatically adopted. Cleanup never detaches or unmounts.

Mounting uses the internal mount implementation with loop, nodev, nosuid and
noatime. Admission checks the exact mountpoint, ext4 UUID, whole-filesystem root,
required options, expected loop block device, image backing device/inode/path,
zero offset, full size and writable mode. Submounts and other aliases of the same
image are rejected, and host headroom is rechecked. Unknown backing metadata is rejection.
The mounted filesystem size must match the capacity range; its Root-owned root is
made private only after identity checks. A valid mounted retry performs no format,
allocation or remount. A known unmounted filesystem is checked before mounting.

Nineteen new regressions bring the focused suite to 232 tests. The lifecycle
fixtures replace Root ownership/authentication, management commands and kernel
mount/loop observations; their small file allocation, registration, inode checks
and exclusive locks are real. A separate ordinary-user 16 MiB file experiment
executes the actual allocation, nondiscard ext4 formatting, readonly filesystem
check and UUID probe, retaining full physical allocation. No real 16 GiB image,
loop attachment, privileged mount, persistent mount unit or live host preparation
is performed. Capacity and reservation are admission checks, not completed
workload/quota acceptance; live full-disk, periodic trim/reservation drift, resource
and recovery scenarios remain pending. The complete guarded upgrade must still
stage reviewed candidate source separately, preserve 659, capture the replacement
Core before revoke/retire, install only reviewed files and define startup/recovery
ordering. This helper does not install code, alter Apps/rules or enable production.

### Candidate independent bootstrap source

The authenticated administrator-only `stage-source` entry and
`stage-protected-source.py` add the fourteenth helper resource. Staging requires
the accepted 659 slot and controller lease to remain occupied, stopped trusted
units, idle UID995 and readonly admission of the same ready staging volume. It
never relaxes the normal `source.prepare` outstanding-slot guard, changes the old
mirror/source/freeze, runs project programs or releases the publication slot.

This bootstrap path is offline. It verifies the protected old source/freeze and
fixed historical baseline, captures a private exact-pinned candidate bundle and
clones the protected old object store without hardlinks. Bundle advertisement must
identify the single reviewed helper revision; prerequisites, ancestry and fetched
object integrity are checked before materialization. Every helper blob in the
candidate must match the authenticated release. NUL-delimited source entries have
explicit type/path/count/byte bounds; links, submodules and the reserved host
overlay are rejected before checkout. Source, retained old freeze/lock, code,
storage and process state are checked again before completing the new freeze.
Existing or partial staging is retained and never automatically reused.

Candidate Git runs with clean configuration/environment, no hooks/fsmonitor,
external protocols or inherited network routing. Private temporary output files
bound controller memory use; replies exceeding 4 MiB are rejected. Each candidate
Git process has a 2 GiB address-space limit, 120 CPU seconds, 128 MiB generated-file
limit, 128 descriptors and a 120-second wall timeout, with one pack thread. These
per-process limits do not constitute live whole-cgroup resource acceptance.
Bundle size is at most 128 MiB and materialized source at most 20,000 files /
256 MiB. Blocked staging grants no installation, quality or delivery authority.

Sixteen source regressions and one readonly-volume-admission regression bring the
focused suite to 249 tests. Ordinary fixtures replace Root ownership/authentication,
old-slot/unit probes and privileged volume admission; actual Git prerequisite
bundles, independent object copies, worktrees, inode/lock/byte checks and bounded
subprocesses execute. The trusted baseline here is the previously authenticated
pilot freeze, not a fresh remote-base observation. The full upgrade and subsequent
delivery must independently revalidate the configured repository/current base;
staging does not bless stale input. Live Root staging, complete package/installation
recovery, full workload/resource/cache/native coverage and App/rule/merge acceptance
remain pending. The accepted 659 installation and normal 25-file pilot package
remain unchanged.

### Candidate bootstrap source release

`package-bootstrap-release.py` exports a separate source-input archive from a clean
committed revision. It neither takes an operator Core build record nor executes or
includes an operator Core binary. The independently reviewed Core crate tree is
required; a future administrator transaction must build/capture that Core under
UID995 and check its policy before installation. Existing normal pilot packaging
and its 25-file inventory are unchanged.

At `c821606`, the archive had 64 exact regular members: fifteen authenticated helpers,
twenty-two proposed installed-file source inputs, twenty-three fixed-659 source
references, and four metadata/bundle members. The accepted 659 installer had
thirteen controller modules and twenty-four flat release members; its old source
reference aggregate is independently pinned. The candidate controller has fourteen
modules. Source references include the old installer/upgrader but no Core binary
or private App keys. The pilot mount and sudo delegation templates are explicitly
retained byte-for-byte. Other proposed destinations, modes and before/after hashes
come from a fixed source map, not archive-supplied paths.

`bootstrap-release.py` is the fifteenth authenticated helper. Administrator-only
`inspect-release` verifies the external archive pin before parsing bounded plain
USTAR headers, without filesystem extraction or project execution. Links, sparse
or extension headers, unknown/duplicate/missing members, noncanonical metadata,
padding/trailing data, scope/provenance changes and changed old/helper digests are
rejected. Bounds are a 160 MiB archive, 128 MiB source bundle and 1 MiB per other
member. Complete payload digests and the exact installed-file plan must match.

Inspection remains input validation. Before any installation, the future upgrader
must supply the external authenticated helper-manifest pin and independently bind
every proposed new file and helper byte to the protected staged Git revision.
Rehashing an operator payload cannot satisfy that Git binding. The archive records
the required preservation/build/capture/reverification-before-revoke/retire order;
the executable transaction/recovery implementation remains separate work. Its
first policy was explicitly a pilot-only template. The Linux transition template
below replaces that candidate input; external archive/data/host-recipe inputs
and captured-Core policy checks remain required, with
`administrator_ready=false` and production disabled.

Fifteen new release-contract regressions bring the focused suite to 264 tests.
Parser/plan tests use an explicit small synthetic old-source fixture; the fixed
production old-source digest is not weakened. Root ownership/authentication is
replaced only for ordinary inspection fixtures. Actual parsing, hashes, file types,
metadata and source-binding rejection execute. These checks do not constitute a
live Root upgrade or completed project verification. The accepted 659 deployment,
Apps, repository rules and services remain untouched; full guarded package/host
preflight, transition policy, installation/recovery and broader acceptance are
still required before any administrator offer or issue closure.

### Candidate fixed-659 preflight

`preflight-history-pilot.py` is the sixteenth authenticated helper, selected by
the administrator-only `preflight-pilot` command. It observes the accepted 659
installation while holding the existing exclusive controller lease. It does not
run validation, revoke or retire a run, create preservation copies or volumes,
install files or change repository rules. Its summary grants no upgrade authority
and retains `administrator_ready=false` and production disabled. A caller can
retain the same lease for a future composed transaction; observations must be
rechecked before that transaction changes state.

The fixed old stage's installer, upgrader and 24-member release are independently
hashed before parsing; neither old script executes. The 23 old source references
and Core provenance must match. Installed bytes, exact modes and module/bin/policy
inventories, current Python/Git pins, exact restricted account/group membership,
stopped workers, accepted report, configuration and occupied journal slot must
agree. Safe Root-owned Python 3.14 caches are bounded data and never imported;
unknown modules, cache names, links and writable inputs block observation. Pilot
mount checks include the UUID, complete backing image inode/device, options and
absence of loop/mount aliases. Operator and installed signing-key copies must
still match; credential bytes are never included in the summary.

Journal queries use a read-only, query-only SQLite transaction with schema,
physical/logical size, row and time bounds, including committed WAL rows. They do
not construct `Journal`, modify logical records, checkpoint or create a missing
WAL shared-memory sidecar. Existing SQLite shared-memory read coordination is not
a claim of a byte-for-byte unchanged SHM file. Configuration policy, old source,
bundle, journal, mount and credential identities are checked again after the
remote observation. The App client comes from authenticated old source blobs,
without installed-module or bytecode imports. It explicitly requests only
`contents:read`, mints the ephemeral installation token using the existing App,
then GETs the repository and main ref. A different repository or advanced base,
unavailable API, unexpected permissions or transport failure blocks; there is no
PAT, `gh`, proxy or alternate-identity fallback. No Check, App configuration or
persistent repository setting is written.

Twenty-one new regressions bring the focused suite to 285 tests. Ordinary fixtures
exercise actual archive parsing, installed-file rejection, JSON, WAL reads,
mutation detection and lease ownership; Root ownership, accounts, credentials,
kernel and remote responses are replaced. Separately, the retained actual 659
release's 24 members and fixed source-reference aggregate parsed successfully as
UID1000 with Root ownership replaced. Neither experiment is live Root preflight
or App acceptance. At `0b8d27e`, the helper closure gave the source archive 65 members;
it contained no Core binary. The Linux template below is a separate candidate;
complete
ingress/storage/transaction composition, installation/startup/recovery and live
acceptance remain required before an administrator offer. Installed 659, existing
Apps/rules and the normal pilot package are unchanged.

### Candidate Linux transition policy

`linux-transition.flow.toml` adds the complete Linux Test adapter to both `full`
and `hook`, alongside the two existing pilot steps. All three external steps
remain required, use the pinned `gatehouse-step` launcher, and run serially;
the normal Core secret scan and architecture audit remain mandatory. This is one
candidate coverage cell, with other Linux quality jobs and native macOS/Windows
coverage still pending. Neither the profile name nor a completed Linux workload
can promote `coverage_acceptance` from transition or authorize publication.

`linux-transition-policy.py` is the seventeenth authenticated helper. Its pure
contract validates the flow and canonical `linux-transition.policy.json` together,
including requiredness, profiles, launcher, report paths and the flow digest.
The inventory specifies fixed history/head/base/receipt/input-manifest arguments,
the existing 85 named capture paths, and required readonly runtime, directory
source, Cargo configuration, test-data archive and onboarding bindings. It never
enumerates onboarding scratch or captures disposable keys. Cache input templates
cover the lockfile, Cargo/nextest configuration, workflow and test-data manifest;
actual toolchain pins, cache identity, verified host bindings and full host
configuration must still be composed from reviewed inputs. The proposed 1,800 s
outer, 1,260 s worker, 8 GiB memory, 1 GiB file and 16 GiB storage bounds are
candidate limits, not measured host resource acceptance. Publication stays blocked,
`host_configuration_complete=false`, `administrator_ready=false` and production
disabled. A policy template cannot be used as a complete host configuration.

The bootstrap source map keeps the immutable 659 source map and 23-source
aggregate separate. The new flow's source name differs from `pilot.flow.toml`,
but its before hash is resolved by the same installed destination. This preserves
the old-file proof during template replacement. Duplicate old/new destinations
are rejected. The candidate has 25 proposed file inputs: the new flow and policy
inventory, the two Linux worker libraries, the current controller and existing
wrappers/presets. Old mount and sudo delegation templates remain byte-identical.
With seventeen helpers, 23 old references and four metadata/bundle entries, the
source-only archive has 69 members and still no Core binary. The normal 25-file
pilot package and accepted 659 installation remain unchanged.

Twelve new policy/mapping regressions bring the focused suite to 297 tests.
They reject weakened requirements, profiles, builtins, launchers, paths, capture
or input bindings, rehashed policy changes, authority promotion and ambiguous
destinations. The already built ordinary-user release Core accepted the actual
new flow and produced both `full --all` and `hook --all` plans with the same five
selected nodes: two builtins and all three external steps. Those commands only
validated configuration and planned execution. The 85 selected names match the
earlier complete transport selection, rather than reducing retained evidence.
No Linux workload was rerun and no Root/systemd/UID995 acceptance, deployment,
App/rule migration or performance improvement is inferred. Protected input
bindings, complete configuration, captured-Core checks, ingress/storage,
transaction/startup/recovery and the remaining coverage still precede any
administrator offer or issue closure.

### Candidate readonly Linux input bindings

The controller now accepts an optional complete `linux_inputs` binding for
directory dependency sources, test-data archives and onboarding source, plus an
externally pinned inventory outside all three trees. It requires the existing
runtime and Cargo configuration bindings. Each role has one fixed readonly
worker destination from the transition policy; no inventory or broader host
directory is mounted into the worker. Missing roles, unpinned metadata, unsafe
mount operands and overlaps between inputs, source, scratch, inbox, cache,
control, mirror, policy or signing-key domains block execution. Existing pilots
without the new binding retain their previous behavior.

Before worker launch and during policy verification, the controller checks the
Root ownership and protected parents, exact readonly file/directory inventory,
streamed hashes, modes, link counts and complete tree. The three roles share
the existing separate input budgets: 512 MiB per file, 6 GiB total, 150,000 files
and 64 MiB metadata. Generic controller readers and runtime mount operands are
unchanged. Relative crate fixture names may contain spaces and punctuation as
data; absolute paths, normalization/parent escapes, backslashes and control
characters remain rejected. They are never shell or systemd operands.

Dependency package names, archive checksums and Cargo directory checksum files
must agree with the frozen source lock and protected role inventory. The exact
compressed test-data bytes must match the frozen manifest. Onboarding must match
the independently reviewed source digest. Cargo configuration must select only
the fixed offline directory replacement. Inventory, manifest and source-lock
facts are read again before completion. These checks consume separately reviewed
Root inventory pins; they are not an authorization to import an Agent-created
inventory or to execute project/package installers as Root. Trusted provisioning,
full configuration and lifecycle composition remain required.

Seventeen new regressions bring the focused suite to 314 tests. They cover missing
and changed roles, content/source/checksum drift, invalid Cargo sources, ambiguous
paths, links, special/writable/missing files, metadata mutation, bounds, legal
crate fixture names and fixed readonly sandbox arguments. An ordinary-user
actual-data experiment verified 205 packages including generated checksum files,
three test-data archives and 469 onboarding files: 9,397 files / 235,706,723 bytes,
alongside the 175 tools / 1,020,148,204 bytes, in 7.035 seconds. It retained the
production source pins and replaced Root ownership only. The first attempt
rejected thirteen legitimate crate fixture names containing punctuation; the
corrected role-only filename check passed in a fresh fixture, with the original
attempt retained privately. No package code ran, and this is not Root import,
mounted-systemd/UID995, workload, deployment, resource or quality acceptance.
At `9e8598a`, the helper/source-release inventories remained seventeen and 69
members, with no Core binary. Installed 659, Apps, rules and services remain unchanged.

### Candidate protected Linux data capture

The eighteenth helper, `seal-linux-inputs.py`, adds administrator-only
`seal-linux-inputs`. It requires the authenticated helper revision to match the
protected Git source, the fixed retained 659 controller lease and occupied slot,
stopped workers, and admission of the separate registered storage volume. It
revalidates the independent official-tool/dependency import before and after
capture. Dependency files are reused only from that verified readonly import.
The source lock and manifest must match protected Git facts; arbitrary existing
directories or an Agent-created inventory cannot substitute for these proofs.

Only a fresh private `bootstrap-linux-inputs` destination is accepted. Compressed
test-data files are copied before using their independently source-pinned hashes
and sizes; the existing archive copier's 128 MiB per-file cap is retained.
Onboarding is copied without following links, executing installers or trusting
mutable source checksums, then checked against the independently reviewed whole
source digest. Files become readonly only in the new destination. Extra/empty
directories, hardlinks, special files, source changes and byte/entry/metadata
budget violations block sealing. Existing role budgets are not widened.

The result includes canonical role metadata outside the three trees, the fixed
offline Cargo configuration and a private binding with their explicit pins.
Source, imported inputs, copied bytes/modes, helper code, storage and the old slot
are checked again before the binding and completion record are written. Partial,
blocked or completed destinations are never implicitly reused. A caller can
retain its exact lease descriptor for later reviewed composition. Failures retain
the private data and old pilot; this helper does not revoke, retire, install,
publish, create an App Check or grant quality authority. Complete configuration,
transaction/recovery and host acceptance remain required; administrator readiness
and production remain false. At `6130de6`, the source archive had 70 members, including
eighteen helpers, with no Core binary; the normal pilot package is unchanged.

Eighteen new regressions bring the focused suite to 332 tests, including a
controller-verifier round trip through actual small Git/copy/hash fixtures and
negative lifecycle/input/reuse cases. Root authentication, ownership, slot,
worker, imported-input and storage boundaries are explicitly replaced in these
ordinary fixtures. Separately, actual copy/inventory primitives captured the
three compressed archives and 469 onboarding files and inventoried the existing
205-package sources in 6.326 seconds. The controller then verified all 9,397 role
files / 235,706,723 bytes plus 175 tools / 1,020,148,204 bytes in 7.120 seconds with
production pins unchanged. This experiment invoked the copy/inventory primitives,
not the privileged entry or its authentication/lease/slot/import/source-Git/mount
lifecycle. Neither it nor the fixtures establish live Root, UID995, workload,
resource, deployment or quality acceptance. Installed 659 remains untouched.

### Candidate complete Linux transition configuration

The nineteenth authenticated helper, `compose-linux-host.py`, adds the
administrator-only `compose-linux-host` entry. It generates an explicit host
configuration and installation plan under the existing 659 controller lease.
It does not overlay arbitrary old settings. The old journal, submission inbox,
dedicated Apps and key paths remain fixed, and authoritative publication remains
blocked in transition mode. The engineering-policy semantics are unchanged.

Before composition, the helper rechecks the actual old-pilot preflight, current
remote base, stopped worker identities and registered separate volume. It
inspects the externally pinned source release and binds its helpers and installed
file inputs to protected Git. The pristine staging record, freeze and captured
bundle must match that release. Imported official tools/dependencies, sealed data
and captured Core each need matching complete records, independent pins and the
same candidate, helper, source and input identities. A binding file beside an
incomplete or blocked sealing record cannot substitute for completion.

Core must be the captured UID995/GID981 release build with incremental compilation
disabled and a proven stopped worker. Its recorded sandbox arguments and
environment must match the fixed build recipe. The source snapshot, host tools,
captured binary bytes and readonly mode are checked again. The ordinary operator
binary is never selected. The source-bound controller common/runtime modules run
from a temporary in-memory namespace, using the bootstrap's strict ownership
guard, to verify all official payload and Linux role bytes. They execute no
project or package code.

The generated configuration has 29 execution pins covering every controller
module, both worker entry and sibling preparation module, policy, captured Core,
OS Python/Git and runtime/data/Cargo metadata. Source, mirror, scratch and cache
move to explicit disjoint paths on the separate volume; the three input roles
remain readonly. The fixed full/hook coverage and 85 Linux captures are retained.
The history probe resolves head/base from the protected run, rather than retaining
659 literals. All thirteen cache dimensions use the existing resolver. The Linux
workload uses its existing debug profile with release incremental marked
not-applicable; adding compiler/profile environment overrides would violate its
workflow parity guard. The separate Core build remains release/non-incremental.
The proposed cache bound is 2 GiB, four entries and one day; it is a candidate
capacity, not a measured resource or retention acceptance.

Only a fresh private destination is accepted. Configuration and installation-plan
digests bind the old configuration, fixed installed destinations and captured
Core; scratch/cache directory creation is merely planned. All input, output and
old-pilot facts are rechecked before an externally pinnable binding and complete
record are emitted. Failures retain blocked private output. No revocation,
retirement, installed configuration change, service startup or Check publication
occurs. Captured-Core config/profile checks, preservation/revocation/retirement,
guarded installation/recovery and live workload acceptance remain explicit
prerequisites; `captured_core_policy_checked`, `administrator_ready` and
production stay false.

Twenty new regressions bring the focused suite to 352 tests. They exercise actual
small Git/input preparation, metadata hashes and controller verification, with
Root/source-release/import/kernel/worker proof boundaries explicitly replaced.
Cases include incomplete records, rehashed identity/authority drift, wrong Core
recipe/binary/mode, runtime data drift, changed OS pins/source freeze, repeated
output, wrong/busy inherited leases and postcheck mutations. These fixtures are
not Root deployment, UID995 workload or quality acceptance. At `59092ee`, the source-only
release had nineteen helpers and 71 members with no Core binary; the normal
pilot package and installed 659 are unchanged.

### Candidate captured-Core policy inspection

The twentieth authenticated helper, `check-core-policy.py`, adds administrator-only
`inspect-core-policy`. Under the exact inherited 659 controller lease, it uses
the configuration composer's new read-only mode to reconstruct and reverify an
externally pinned completed configuration. That mode also checks exact private
file modes, the complete control record, installed plan and all original
source/import/seal/Core/host/storage/old-pilot proofs. It neither reuses partial
output nor rewrites a previously completed candidate on failure. The public
composition entry still accepts only fresh output.

Inspection makes a fresh independent Git mirror and working tree from protected
inputs, without hardlinks or alternates. The exact reviewed candidate bundle is
verified and fetched explicitly, including when the candidate is not reachable
from the original mirror's refs. Policy is overlaid only in this private copy:
Core requires the configuration inside the project root, and leaving an overlay
in the real frozen source would prevent a later fresh execution. Source, Git and
policy metadata are inventoried and checked again after each command.

Only three commands are permitted: configuration check in JSON mode, and full
and hook verification plans with `--all --plan --json`. They use the separately
verified captured release Core through transient parser units with no effective
capabilities, network, mutable system view or access to the existing credentials
and pilot control state. The source view and Core mounts are readonly. Each
command has a 30-second runtime, 1 GiB memory and 2 MiB output limit; the outer
forwarder also has bounded files, CPU, descriptors and timeout. A clean explicit
environment replaces manager/user environment. Unit identity/argv and private
stdout/stderr remain available for diagnosis. The unit is stopped and its cgroup
checked even after failure; unknown stop state never becomes acceptance.

Inspection mount targets remain inside private `/work`. The Core builder also
uses a private `/opt/gatehouse` temporary view for its existing toolchain mount,
preserving the old installation inventory while keeping the approved tool paths
and build arguments. The mount destinations must exist or be creatable under
[systemd's mount rules](https://www.freedesktop.org/software/systemd/man/latest/systemd.exec.html);
these private parents keep that preparation within the service view.

The reply validator requires a complete typed configuration report and the exact
versioned passive-plan envelope. Both profiles must select all five nodes with
the mandatory builtin chain and unchanged components/profile inventory. Services,
waivers, shortened coverage, diagnostics, claimed runtime validation or a
verification result block inspection. Raw policy bytes and Core's effective
configuration digest are separate reviewed pins: Core hashes its Rust TOML
serialization including defaults. The validator does not reinterpret that digest
as a raw file hash or accept a value chosen by the reply. The reviewed Core crate
tree remains unchanged.

All configuration and protected input facts are rechecked before completion, as
are copied view metadata, response bytes and successful unit-stop records. An
externally pinnable inspection binding records compatibility of captured Core
with this policy only. It does not change the original configuration binding,
enable authoritative publication, run project steps, grant complete coverage or
install anything. Preservation/revocation/retirement, guarded installation and
startup recovery, live UID995/Linux workload, other quality/native cells and
cache/resource/App/rule/delivery acceptance remain pending. Engineering-policy
semantics, generic limits and quality authority are unchanged.

Thirty-five new regressions bring the focused suite to 387 tests: fifteen passive
protocol cases, four read-only composition cases and sixteen inspection/copy/unit
cases. Root/provenance/OS/kernel boundaries are explicitly replaced in fixtures.
An ordinary UID1000 experiment independently copied the actual `59092ee` source
plus policy, 1,665 files / 13,117,921 bytes and a separate 5,093,893-byte Git store,
in 1.530 seconds. Existing operator Core then passed configuration and both passive
plan checks in about 0.019/0.012/0.012 seconds, with matching view inventories.
That experiment exercised copy/inventory and passive diagnostics only; it did
not invoke Root authentication, record provenance, lease/slot/mount/cgroup or
UID995 admission. It is not captured-Core, workload or quality acceptance. The
source-only release at `88e5932` had twenty helpers and 72 members with no Core binary;
administrator readiness and production remain false, and installed 659 is untouched.

### Candidate installation barrier primitives

The twenty-first packaged helper,
[`install-transaction.py`](../tools/trusted_gate/deploy/install-transaction.py),
provides internal preparation and entry-fencing functions for a future authenticated
installation coordinator. It has no administrator CLI, activation, revocation,
retirement, file replacement or journal-restoration operation. That coordinator
must first reverify the complete release, retained pilot state, captured Core,
configuration/inspection receipts and current base under the continuous controller
lease. These primitives alone do not admit an installation.

Preparation requires the protected Root identity, exact existing lease inode,
both explicit entry pins, original mode 0755 and single-link protected regular
files. It records their original inode/owner/size/mtime and byte digests, current
configuration digest and target generation in a private durable intent. A separate
private barrier binds that intent's plan. Fencing validates every entry before
mutation, records progress, removes all executable bits from the original inodes
through checked descriptors and fsyncs them. It retains source bytes and paths.
Recovery accepts only those original pinned entries in the exact known partial
modes; changed configuration, unknown/replaced inputs, missing/advanced barriers
or a reopened completed fence stop progress. Completed fencing is read-only on
retry. No operation enables entry or restores an older transaction phase.

Changing the original inode matters: an ordinary UID1000 experiment could execute
an already open descriptor after its pathname was replaced with a mode-000 file.
Removing the original inode's executable bits blocked that descriptor too.
Linux documents descriptor execution in
[`execveat(2)`](https://man7.org/linux/man-pages/man2/execveat.2.html), and its
[permission rules](https://www.man7.org/linux/man-pages/man7/path_resolution.7.html)
require an executable bit even for the usual Root capability override. Already
running controllers and their children still require separate drain proof.
The receipt therefore keeps `controller_drain_proven` and `administrator_ready`
false. This experiment did not exercise sudo, Root process admission or deployment.

The installed-source bootstrap now checks the fixed private installation barrier
before reading configuration or importing controller modules, and rechecks it
throughout snapshot use. Any incomplete/unknown phase, malformed/noncanonical
record, link, wrong mode or changed identity rejects startup. An active-generation
record must match the actual configuration bytes and source head. Appearance,
disappearance or replacement of the barrier invalidates an existing snapshot.
Ordinary clean installation with no transaction record remains supported.

An independent SQLite fixture showed why installation recovery must preserve the
current journal: the saved snapshot still held a pending slot after the current
fixture advanced to revoked/retired. The barrier library never opens that database
or restores it. Full coordination must use phase-specific applicability checks
after retirement; fixed-659 occupied-slot preflight cannot be reused unchanged.
Root process drain, complete guarded activation/recovery, actual UID995 workloads
and the remaining issue-98 acceptance are pending. The focused suite has 438 tests,
including seven new bootstrap-barrier cases and twenty fence/recovery cases.
Root ownership and DAC admission are explicitly replaced in these ordinary tests;
actual descriptor chmod/fsync, private records, inode changes and leases are exercised.
The source-only release at `43ca6e2` had twenty-one helpers and 73 members without a Core binary.
Installed 659, delegated operations, Apps, services, rules and quality authority are unchanged.

### Candidate controller process observations

The twenty-second packaged library,
[`controller-drain.py`](../tools/trusted_gate/deploy/controller-drain.py), provides
an authenticated read-only pre-install observation under the original controller
lease. The entry fence must already be complete. Its new `fenced` reader verifies
the original entry inodes are still mode 000, the private receipt/barrier agree
and the old configuration is unchanged; it never completes a partial fence.
The observer verifies the helper closure before and after, checks stopped
Core/worker/bootstrap units, reads two process snapshots and rechecks the fence
between and after them. It writes no record, changes no permissions and provides
no administrator CLI, process signalling, installation or activation operation.

The observer requires the native Linux/x86_64 Root context, kernel procfs type,
matching PID/user/mount namespaces with PID1 and an initial UID map. Process
counts, individual reads, total bytes and elapsed work are bounded. Each numeric
PID directory is opened without following links; subsequent reads use that
descriptor. The parser separates the parenthesized process name before locating
start time, checks PID/Tgid/PPid/namespace consistency and reads all four UID/GID
values. The Linux interfaces specify
[start time in clock ticks](https://man7.org/linux/man-pages/man5/proc_pid_stat.5.html)
and [real/effective/saved/filesystem identities](https://www.man7.org/linux/man-pages/man5/proc_pid_status.5.html).
The four-UID check covers sudo-launched Root controllers as well as active
restricted identities 999/995. Kernel threads and dead processes have explicit
rules; unreadable, disappeared, recycled or inconsistent processes stop admission.

For Root or restricted processes, the classifier checks the reviewed host paths,
Git working directories, signing-key operands, executable paths and fixed
GateHouse unit names in argv/cgroup. It repeats relevant reads to reject exec,
credential or path changes. Only the authenticated caller is excluded as a
controller, after confirming its live Root identity. It never reads environments,
and the returned snapshot uses identity digests instead of raw argv/paths.
Changed PID membership or start times between scans, namespaces, populated units,
changed fence or changed helper bytes reject the result. Unrelated processes are
not terminated or silently added to a permissive whitelist.

The result describes two empty observations at that moment. It keeps
`controller_drain_proven`, `administrator_ready` and production false: the complete
coordinator must repeat observations around later transitions and preserve the
same lease/fence. Post-retirement/install applicability and activation require
their own guards. An ordinary experiment with two owned child processes confirmed
stable PID/start-time and parent observations; a name containing spaces and
parentheses demonstrated why naive whitespace indexing is unsafe. It did not
scan Root processes or prove host quiescence. Twenty-eight new regressions bring
the focused suite to 466 tests, including parser/classification/race/budget/namespace
and authenticated observation cases plus two read-only fence checks. Root/kernel
admission is explicitly replaced in fixtures. That source-only release
has twenty-two helpers and 74 members, no Core binary, and no administrator-ready
installer. Installed 659 and the remaining issue-98 acceptance are unchanged.

The internal `compose-linux-host.py` prepared replay rechecks completed materials
without replaying the live fixed-659 preflight. That preflight requires the old
installed files and pending source slot; it cannot admit recovery after the
coordinator has deliberately fenced entries, revoked the run or retired its source.
The original composition path retains those live guards. Both paths now share
the same source, imported-tool/dependency, sealed-data, captured-Core, runtime and
deterministic configuration/27-file plan verification.

Prepared replay requires an inherited original controller lease, authenticated
helper closure and separate admitted staging storage, with worker identity and
stopped/idle checks. It authenticates the externally pinned complete preservation,
including sealed raw failures and the self-contained SQLite backup. Saved host
configuration must match the authenticated accepted-659 release pins, App/storage
identity, acceptance report and saved run policy digest. It rechecks these facts
and all completed configuration outputs before returning; extra output files,
incomplete control records, altered bytes/modes or authority flags reject replay.
The current journal, installed configuration and retired old source are not read
or restored by this operation. The retained pending journal is historical evidence;
the coordinator must separately verify the live journal's forward transitions.

The returned result explicitly keeps installation admission, captured-Core policy
acceptance, administrator readiness and production false. It does not authorize
revocation, file replacement, activation, quality publication or a rollback. Current
base, current phase/entries, controller drain, completed passive inspection and
retained publication still require independent admission. No administrator CLI is
added. Twenty-one new ordinary-fixture regressions exercise real retained trees,
SQLite state, accepted configuration semantics and runtime hashes, including a
revoked live journal and missing old source while the pinned historical snapshot
remains pending. Root/release/mount/worker boundaries are replaced in these tests;
they do not constitute a live installation or actual-995 workload acceptance.

`check-core-policy.py` also provides an internal completed-inspection replay for
the later coordinator. It requires the inherited original lease and repeats the
prepared-configuration replay around inspection validation. It starts no parser
units, stops no processes and writes no files. The original inspection producer
and this reader share one deterministic command constructor for the three fixed
configuration/full/hook commands and their complete systemd isolation, limits,
mounts and clean environment. Retained invocations must have distinct canonical
UUID4 unit names, exact command bytes, typed successful exit/stop fields and a
fresh stopped-unit observation; a saved stop claim alone is insufficient.

The reader checks the exact private file inventory and modes, externally pinned
completed binding and canonical control record. It reparses bounded stdout with
the existing strict configuration/plan validators and requires empty stderr. The
copied source and independent Git store remain protected, the source is readonly,
all five policy overlay files match the authenticated release, the working tree
has only those deliberate additions and the candidate bundle matches. Its full
view inventory must still match the retained manifest and bound receipt. Rehashed
records cannot introduce weaker isolation, waived nodes, another policy, altered
tracked inputs or authority flags. Time recorded by the producer is observational
metadata and grants no acceptance.

Twenty-four new regressions bring the focused suite to 511 tests. They include
small actual Git-store/view checks and changed readonly modes, policies, tracked
files and bundles, alongside invocation/reply/receipt/provenance/race cases. Root,
prepared-state authentication and live unit boundaries are explicitly replaced.
No Root inspection, actual executor workload or installation is accepted here.
The result confirms passive inspection only and keeps installation admission,
administrator readiness and production false. Phase/base/journal/entry/drain and
publication-retention admission, guarded file installation, activation and recovery
still precede a complete coordinator. That source-only release retained twenty-two
helpers and 74 members without a Core binary or administrator CLI.

The twenty-third authenticated library, `admit-upgrade-journal.py`, provides a
read-only observation of the live old-run journal for the later coordinator.
It requires the inherited original controller lease, matching protected helper
release and pinned complete preservation, plus worker identity and stopped/idle
checks. A query-only SQLite transaction reads committed WAL pages with physical,
logical and time budgets. Missing WAL shared-memory state rejects observation
before SQLite can create a sidecar. It never constructs the mutating `Journal`
class or restores its saved backup over live state.

The saved journal remains historical evidence. Its schema and complete event
prefix must remain unchanged; the live run rows must exactly match replay of the
appended suffix. Only the fixed old run may receive the reviewed invalid/revoking
intent, a stable positive Check ID, revoked completion and ordered retirement
intent/completion events. Another run, altered prior events or inputs, unknown
fields/reasons/operations, sequence gaps, backwards transitions or row changes
without matching events reject the observation. Retirement requires revoked state
and the fixed retained-publication destination. Source, bundle and freeze removal
may proceed only in the source-retirement implementation's order; pending states
retain all three, while completed retirement requires all three absent. Present
source inputs remain bound to the saved bundle/freeze and unchanged tracked Git
source. The journal, source presence, preservation, helper closure and lease are
rechecked before returning.

Twenty-five new regressions bring the focused suite to 536 tests. Ordinary fixtures
exercise real WAL/backup/history, a mocked API failure that leaves the actual
Control in revoking state, and actual Control/Journal/Git worktree retirement
while the saved database remains pending and raw failures remain retained. Root,
helper identity and worker boundaries are replaced; GitHub calls are mocks.
A locally recorded revoked state does not prove a remote API effect. Returned
remote-confirmation, retained-publication, installation-admission, administrator
and production flags remain false. Current remote state/base, full publication
copy, entry generation/drain, file installation, activation and recovery still
need coordinator integration. There is no administrator CLI or installed change.
The source-only closure now has 23 helpers and 75 members, without a Core binary.

The twenty-fourth authenticated library, `install-reviewed-files.py`, supplies
internal file replacement primitives for the later coordinator. Source-release
validation maps exactly 25 reviewed destinations plus captured Core and host
configuration. It binds before/after bytes, modes, retained inputs, source head
and Core/configuration hashes with bounded buffers. The inherited controller
lease, original completed entry fence and unchanged inactive barrier bind a
durable installation receipt. Both delegated entries remain mode `000`, including
their original inodes held by existing descriptors. Core and configuration are
installed after ordinary source files; the two disabled entry files come last.

Before each transition the caller must repeat authenticated live remote,
publication, journal, storage, process and generation admission. The library
then checks the entire recorded generation: original identities for future
files, ordered completed identities, and at most one durable inflight intent.
Three fresh directories have fixed modes and bounded known contents. Exclusive
temporary writes are rechecked before replacement; file and parent-directory
sync precede completion records. Recovery accepts only the known before state,
an empty directory covered by its intent, or exact intended file bytes/mode
after an interrupted rename. It syncs an adopted rename before recording it.
Unknown files, changed identities, altered materials, symlinks, metadata
temporaries, retained file temporaries, exposed receipts, reopened entries and
advanced barriers block further mutation and remain available for diagnosis.

Twenty-nine new ordinary regressions bring the focused suite to 565 tests.
They exercise actual exclusive writes, modes, fsync, replacement, receipts and
crashes around intent, directory creation, temporary creation and rename, plus
changed fresh temporaries and a Core larger than the source-file limit. Root
ownership/DAC and coordinator admission are replaced within private fixtures.
Completion is `installed-disabled`; completed retry is read-only. The library
does not authenticate remote or publication effects and exposes no administrator
CLI, activation or quality acceptance. A complete coordinator must supply those
checks and guarded activation/recovery. Installation acceptance, administrator
readiness and production remain false; installed 659 is untouched. The source-only
closure has 24 helpers and 76 members without a Core binary.

The twenty-fifth authenticated library, `admit-upgrade-effects.py`, observes the
post-revocation prerequisites under the inherited controller lease. It replays
the live journal against complete pinned preservation and requires a locally
revoked, retiring or retired phase. Saved configuration and the accepted 659
release are authenticated independently of installed controller modules. The
old GitHub client is compiled from authenticated source buffers and receives
only `contents:read` and `checks:read` token permissions. It retains its isolated
transport without inheriting an environment proxy. Token references are cleared
on both success and failure.

The caller may supply an explicit canonical HTTP(S) proxy with host and port
from its authenticated Root recipe. Credentials, paths, query/fragment components,
unsupported schemes and malformed ports are rejected. Only HTTPS requests receive
that proxy; existing TLS verification and redirect rejection remain intact. The
returned route digest must be bound by the future coordinator receipt. Omitting
the argument keeps the accepted client's direct transport, regardless of Agent
or operator environment variables. No installed proxy configuration is changed.

Two readbacks bind repository identity, current main baseline, old commit/tree
and the old run's exact authoritative Check. A present Check must match App,
name, head, run and any persisted ID, with completed/failure state. The separate
accepted negative pilot Check cannot substitute for it. Absence is admissible
only without a persisted ID; ambiguous, successful, running, changed or missing
persisted Checks and API failures block observation. A local revoked event alone
never establishes the remote effect.

Registered publication is checked against the entire invocation subtree in
pinned preservation, including private diagnostics and empty directories. The
recorded invocation path, run, machine/manifest/registry digests and any saved
configuration digest/policy must agree. The destination must contain the exact
files, directories, modes and bytes without links, within entry and byte budgets.
An existing partial copy is rejected before later source retirement. Without
registered evidence, raw preservation is still required and an unexplained
publication directory blocks observation. Copy equality confirms preservation
only; it grants no Core quality verdict or delivery acceptance.

Thirty-one new regressions bring the focused suite to 596 tests. They use real
private preservation trees, WAL snapshots and Git worktree retirement, and the
accepted source-loaded client's actual Check lookup with mocked API responses.
Root ownership, accepted-stage/configuration authentication and worker admission
are explicitly replaced. Remote state, copy contents, journal, retention,
configuration, helper identity, lease, worker state and time budget are rechecked.
No actual Root/GitHub effect acceptance is claimed. The observer performs no
Check mutation, copying, retirement, installation or activation, and has no
administrator CLI. Full preparation, storage and entry/drain admission remain
required; installation, administrator and production flags stay false. Installed
659 is untouched. The source-only closure has 25 helpers and 77 members, without
a Core binary.

The twenty-sixth authenticated library, `observe-installation-inputs.py`, combines
the disabled-entry prerequisites around one sealed, SHA-pinned Root recipe and
the inherited original controller lease. The recipe has a closed schema for
source/archive, host, volume and preservation paths plus preparation, seal, Core,
configuration, inspection and retention pins. It also binds the explicit API
route through the effect observer's transport digest. Unknown verdict fields,
wrong revisions, pins, permissions or linked inputs reject observation.

The observer replays completed preparation/passive inspection, reconstructs all
27 file materials from authenticated release and capture bytes, and requires
the old run's remote/copy observation to be in retired phase. It performs two
bounded native process scans, rechecks namespace identity, and requires reconcile
service/timer units and drop-ins to be absent on disk and absent/inactive in the
manager without triggers or pending reload. It changes no units. Remote effects,
preparation, materials, recipe, entry generation, worker state, lease and time
budget are rechecked before return. These are momentary observations; no durable
controller-drain claim or installation authorization is returned.

Phase checks distinguish the original fenced generation before file installation,
an immediately enclosing file-engine checkpoint, and full installed-disabled
replay. Original `fenced()` is never reused after legitimate replacement or
configuration changes. Inside a file checkpoint, the outer observer validates live
prerequisites and generation context; the immediately enclosing engine retains
responsibility for its complete file inventory and fresh temporary proof. The
observer never creates or adopts that proof. Standalone completion instead runs
the engine's full inspection with no temporary exception. A retained temporary
therefore still blocks retry even when other live prerequisites pass.

Twenty-seven new regressions bring the focused suite to 623 tests. They exercise
the actual 27-file transaction with the live-observer callback, private proc-file
parsing, sealed recipes, full source-release decode/materialization, file modes,
receipts and recovery rejection. Preparation/capture authentication, Root/DAC,
native namespaces, systemd and API responses are replaced within fixtures; this
is not actual Root or remote acceptance. No administrator CLI or activation is
exposed. Installation, administrator and production flags remain false, installed
659 is untouched, and a complete coordinator plus live acceptance remains required.
The source-only closure has 26 helpers and 78 members without a Core binary.

Record actual executor identity/platform, exact command/profile/configuration,
policy/toolchain digest, expected step IDs, evidence location, negative cases,
acceptance revision and responsible owner for each migrated cell. Update the
protected host coverage inventory from that reviewed matrix. An empty matrix or
an unassigned applicable row keeps `coverage_acceptance` in transition mode.

The separate Rust/frontend cache acceptance must record real project/revision,
full commands, cold/warm/source/toolchain/dependency/configuration scenarios,
compilation/test/maintenance wall time, logical/allocated disk and remote reporting
latency. No such production measurements are claimed by this inventory.
