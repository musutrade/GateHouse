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

The accepted `659803a` pilot remains on its existing 2 GiB volume. Neither this
preparation helper nor its ordinary-user observations install inputs or modify
that filesystem. A future reviewed upgrade must preserve its outstanding source,
journal and canonical publication before retirement, import independently checked
inputs into a separate bounded filesystem, select the full Linux Test adapter
and its 85 capture paths, and set measured resource budgets. Toolchain, scratch,
persistent cache and evidence remain separate; capacity availability alone is not
quota approval or protected-worker acceptance. No old installer/fa18 upgrader is
valid for this transition. All matrix owners remain unassigned until live checks.

Record actual executor identity/platform, exact command/profile/configuration,
policy/toolchain digest, expected step IDs, evidence location, negative cases,
acceptance revision and responsible owner for each migrated cell. Update the
protected host coverage inventory from that reviewed matrix. An empty matrix or
an unassigned applicable row keeps `coverage_acceptance` in transition mode.

The separate Rust/frontend cache acceptance must record real project/revision,
full commands, cold/warm/source/toolchain/dependency/configuration scenarios,
compilation/test/maintenance wall time, logical/allocated disk and remote reporting
latency. No such production measurements are claimed by this inventory.
