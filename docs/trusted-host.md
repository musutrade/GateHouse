# Trusted local verification and GitHub Checks

Implementation for [#98](https://github.com/musutrade/GateHouse/issues/98).
Engineering Policy semantics are unchanged. Rust Core owns the project decision,
requiredness, quality thresholds, waivers, debt, baselines and ratchets. The Python
host materializes inputs, isolates commands, retains Core publications and delivers
that decision. It contains no quality evaluation engine.

This implementation is a **deployment candidate**, not an accepted migration of
GateHouse's current CI. Existing required jobs and workflow triggers remain in
place. Deployment, native platform coverage, real project cache measurements and
the live negative acceptance below must pass before #98 can close. Nothing here
adds an Actions signature verifier or a failure fallback to Actions.

## Execution and publication boundary

The installed controller, policy, Core executable, Git mirror, candidate, journal,
App keys and evidence belong to root, outside the developer account. Every parent
of a trusted path must be root owned and not group/other writable. State and keys
are private. A developer-owned 0700 directory does not meet this boundary.

Use an independent non-login executor account with no sudo, Docker, disk or other
privileged group membership. Core and its report writer stay in the trusted
process. Every external flow step must be `gatehouse-step` with one fixed step ID;
that root-owned launcher accepts no arbitrary program from a caller. It starts
only the administrator-configured argv under the executor account. The worker
has a read-only source mount, separate writable scratch/build-cache mounts, no
host home, control directory, App key or system sockets, no network or elevated
capabilities, and a bounded systemd cgroup. Worker units bind their lifetime to
the Core unit. Core records exit results and writes its own protected reports.

The initial Linux executor supports serial command steps and Core's existing
secret scan/architecture audit. It rejects source symlinks, submodules and
privileged services/runner integrations. Projects needing these capabilities need
an explicitly reviewed materializer/launcher before their coverage can migrate.
Unsupported requirements must keep their existing CI owner.

Private directories, cgroup options and offline unit tests alone do not establish
the deployed boundary. Test it with the actual developer and worker identities.
Apply a filesystem quota to writable scratch/cache storage; a per-file size limit
does not bound total disk consumption. Do not grant the developer permission to
edit the installed package, run the controller as root, use a privileged container
socket, or invoke arbitrary systemd units.

The 2026-10-17 local inspection found that the current development account belongs
to Docker/LXD and administrative groups and can access the Docker socket. The
controller refuses that account as its production developer identity. Root-owned
directories on the same host do not establish separation from a developer with
host administration capabilities. Use an independently constrained development
identity/environment or a separate trusted host, and verify its actual access.
Docker documents the [root-level privilege of its group](https://docs.docker.com/engine/install/linux-postinstall/#manage-docker-as-a-non-root-user).

## Durable protocol

Install the reviewed `tools/trusted_gate` package at
`/opt/gatehouse/trusted_gate`, the operator configuration at
`/etc/gatehouse/host.json`, and the reviewed policy separately. The administrative
entry is `/usr/bin/python3 -I /opt/gatehouse/trusted_gate/host.py`. It must run
through the protected operator/service boundary; it is not an Agent shell hook.
The [example settings](../tools/trusted_gate/deploy/host.example.json) deliberately
use `coverage_acceptance: "transition"` and an empty pin inventory, which cannot
publish a successful authoritative Check.

1. The consumer confirms its previous batch merged, selects the latest base and
   freezes the next task members/order. Agent scheduling and integration belong
   to herdr-issue-flow, not this host.
2. Export the candidate as a Git bundle. The protected `validate --bundle PATH
   --head FULL_SHA` entry independently fetches the configured repository's
   current base, imports bounded content-addressed objects without Agent Git
   config/hooks, verifies base ancestry and materializes a root-owned checkout.
   Reserved `.gatehouse-host` paths cannot be supplied by the candidate.
3. Core runs `verify --profile full --all` with the separately reviewed flow
   over that candidate. The trusted overlay is an explicit policy input and is
   not represented as a change to the Git tree. Pin all installed controller
   modules, Core, launchers, policy/configuration and toolchain executables.
   The entire host configuration participates in policy identity.
4. The journal records local `pending -> running -> passed|failed`. A successful
   result needs Core's complete PASS publication, exact invocation/commit/profile,
   the whole expected step inventory, successful non-waived execution, and intact
   machine result, registry, manifest and all declared artifacts. It cannot be
   registered from an Agent-provided Boolean or a transport receipt.
5. `deliver RUN --title TEXT --body-file PATH` uses a **different delivery App**
   to push only that accepted SHA. Its deterministic `gatehouse/RUN` branch and
   PR query recover interrupted pushes/PR creation. No force push is used.
   `attach RUN --pr NUMBER` supports an existing protected publisher instead.
6. After GitHub contains the SHA, `publish RUN` checks repository ID, PR head,
   baseline, remote tree, current policy/evidence and strict, App-bound required
   rules. It first establishes `in_progress`, then updates the same Check to
   `success`. Only complete valid local PASS can produce success. Failure and
   revocation use `failure`; neutral/skipped/stale are never used to grant delivery.
7. `reconcile` checks current applicability and remote Check identity. It records
   `merged` only after GitHub confirms merge and the landed tree and first-parent
   base match the verified input. Merge and squash are supported; rebase requires
   a separate equivalence contract. Until confirmation the publication slot stays
   occupied. `retire RUN` retains the publication before removing the root-owned
   source slot, and requires a terminal result and stopped executor cgroups.

The SQLite journal uses immediate write transactions, FULL synchronization and a
separate event sequence. Run epochs fence older operations. A repeated identical
request returns the same run; a failed/interrupted attempt is not silently rerun.
A fresh attempt requires confirmed stop/revocation and a new epoch.

GitHub API failures are unknown external outcomes. They preserve the local
result and operation for a bounded retry. A lost Check-creation response is
recovered by listing the same App/name/SHA, never blindly creating another Check.
Ambiguous multiple own Checks block recovery. A lost success response can be
retried without rerunning Core. No retry creates another PR or grants a new PASS.

`revoke RUN --reason TEXT` first records local invalidation, then changes the
remote Check to failure. Failed remote revocation remains `revoking` and holds
the slot. A stale operation cannot overwrite a later epoch. The operator must
confirm stopped cgroups before reclaiming an interrupted run; elapsed time is
not confirmation that a process stopped.

## App permissions and rules

The Checks App needs installation permissions `checks:write`, `contents:read`,
`pull_requests:read`, plus GitHub's metadata read access. The delivery App requests
only `contents:write` and `pull_requests:write`. Both token requests are restricted
to this repository. Tokens stay in protected process memory; Git fetch/push uses
an isolated root Git environment, never a token in argv or a credential helper.
The transport does not use `gh`, PATs, ambient Git credentials or Agent text as a
fallback. It rejects redirects and does not print response bodies or headers on
API errors.

Use an active strict required check with the **actual Checks App integration ID**.
For a ruleset, the relevant parameters are:

```json
{
  "strict_required_status_checks_policy": true,
  "do_not_enforce_on_create": false,
  "required_status_checks": [
    {"context": "GateHouse Trusted Verification", "integration_id": 4867361}
  ]
}
```

This is a configuration example, not an instruction to replace GateHouse's
current rule before acceptance. Preserve the no-bypass rule, independent review
requirements and actual merge authorization. Checks do not prevent local commits
or ordinary pushes; the protected delivery entry and credentials enforce the
submission boundary.

Provision `evidence_url_prefix` on the existing protected evidence service before
enabling complete mode. Each Check's details URL links its durable run record.
This package does not start a web server or publish raw evidence to GitHub.

On 2026-10-17, read-only App-authenticated inspection found:

| Identity | App / installation | Observed scope |
| --- | --- | --- |
| Existing gate App `my-disposable-bot` | 4867361 / 159921491 | Organization-wide installation, including Checks write; its App-level permissions are broader than the narrowly requested runtime token |
| Existing delivery App `musutrade-symphony-delivery` | 5069731 / 164696480 | Contents/PR write; installation currently selects only `musutrade/disposable` and `musutrade/disposable-2` |

The discovered legacy private keys are readable by the developer account. Merely
copying a key into `/etc` would leave that independent signing capability intact.
Provision an exclusively protected key and retire developer-readable signing
copies (or create a separate App), with an explicit rotation plan for existing
consumers. Add the intended repository to the delivery installation if that
identity is selected. Do not change other consumers' permissions implicitly.

## Migration, invalidation and rollback

Create a reviewed matrix of check × native environment × configuration, with
trigger, exact command, expected evidence and unique owner. Include the entire
Test job (Rust, findings/artifact tests, workflow acceptance, schema checks and
locked real plugin onboarding), native macOS/Windows tests/builds/contracts,
format, Clippy, security database identity/freshness, all fast/risk/matrix/coverage
and critical-path work, CLI contracts, documentation, release contracts and
quality script tests. The current reference is [ci.yml](../.github/workflows/ci.yml).
Advisory and scheduled trend work retains its own policy.

Do not mark `coverage_acceptance` complete while any applicable required cell is
unowned or only demonstrated on another platform. Only after equivalent coverage
and the live negatives pass may the reviewed migration remove the corresponding
PR/push validation triggers and replace the required aggregate with the App-bound
Check. A transition retaining native Actions is a transition, not full local mode.

The [reconciliation timer](../tools/trusted_gate/deploy/gatehouse-reconcile.timer)
is a bounded service invocation, not an Agent poller. Configure its cadence and
run `reconcile` on policy changes, evidence withdrawal and reruns. Administrative
policy/toolchain updates must first quiesce publication and confirm remote
revocation of active successes, then install/activate the new inputs. A timer
alone cannot make an administrator's concurrent policy change atomic with GitHub
merging. Strict baseline enforcement addresses the corresponding base-update
race, but read-before/write-after API checks are not an atomic merge transaction.

Rollback explicitly restores the prior required rule and workflow triggers,
disables new publication, revokes active external successes and confirms remote
state. It preserves evidence and baseline lineage. It does not silently run old
Actions as the new mode's failure branch.

## Persistent cache and acceptance

`cache.py` partitions local acceleration by repository, native platform/architecture,
toolchain, target, profile, features, flags, dependency/build-configuration digests,
instrumentation, explicit release incremental setting and executor identity. Rust
release incremental compilation must be inspected explicitly. A compatible target
directory is retained across batches; no cache hit supplies a gate result. The
current frozen candidate still executes every required command.

Each step declares `cache_inputs.dependencies`, `cache_inputs.configuration` and
`toolchain_files`. The host hashes those actual files, native platform, executor,
argv and environment for each candidate; copied/stale digest labels are not used.
Declare all manifests, lockfiles and relevant build configuration, including
currently absent optional configuration files. For release work, explicitly set
`CARGO_PROFILE_RELEASE_INCREMENTAL` to the declared `true`/`false` value. Changes
to source still execute the gate and rely on the native build system's source
fingerprints within the compatible namespace.

Caches and scratch are separate from protected reports and state. Never import
the Agent's writable target/package/framework cache into this trust domain. Cache
maintenance runs under the controller lock, rejects unknown/symlinked entries,
preserves active entries and enforces configured entry/byte/age bounds. Source
reports, coverage databases and test run data are not cache hits. Projects must
declare fresh run output paths. Frontend integration must use its actual package
and framework cache interface, not assume Cargo settings apply to it.

Provision the root-controlled Cargo registry (and optional Git dependency cache)
separately. Workers bind those dependency sources read-only beneath a per-step
writable Cargo home, so Cargo's package locks do not require a writable shared
registry. Compiler outputs use the compatible persistent target namespace. Use
actual installed toolchain executables rather than developer-owned rustup proxies.

Run the focused regression suite with:

```sh
python3 -B -m unittest discover -s tools/quality/tests -p test_trusted_gate.py -v
```

Before production migration, retain evidence for:

- Real protected host PASS, App-source read-back, GitHub merge and landed input;
  no equivalent Actions project validation or additional verification job.
- Real missing check, wrong SHA/base/policy, wrong App/same-name forgery, partial
  profile, skipped/neutral and withdrawn result: all stay blocked.
- Actual developer/worker inability to read keys, modify policy/state, gain rule
  administration or bypass/forge success. Offline fixtures cannot prove this.
- Lost POST/PATCH response, duplicate event, report retry and revocation retry;
  a pushed candidate resumes delivery once without another full validation/PR.
- Single and combined snapshots, conflict repair producing a new candidate,
  external base advance, and previous-batch merge confirmation before next freeze.
- Actual Rust and frontend cold/warm caches, source changes and toolchain,
  dependency/configuration changes, retaining compilation/test/maintenance time,
  total wall/resource costs, disk use and reporting latency. No speedup is assumed.

Protocol references: [Checks API](https://docs.github.com/en/rest/checks/runs),
[required checks and App source](https://docs.github.com/en/repositories/configuring-branches-and-merges-in-your-repository/managing-protected-branches/about-protected-branches#require-status-checks-before-merging),
[Cargo build cache](https://doc.rust-lang.org/cargo/reference/build-cache.html).
Sandbox property semantics follow [systemd.exec](https://github.com/systemd/systemd/blob/main/man/systemd.exec.xml).
