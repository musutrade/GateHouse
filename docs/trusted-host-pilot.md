# Install and observe the restricted-identity pilot

This is the next stage of the [deployment plan](trusted-host-deployment.md), after
the administrator has provisioned `gatehouse-developer` / `gatehouse-executor`
and added GateHouse to the existing delivery App installation. It is a bounded
Linux isolation pilot. It does not complete [#98](https://github.com/musutrade/GateHouse/issues/98)
or replace the current required CI.

## Reviewed release and administrator entry

The developer prepares a local immutable release using
[package-pilot.py](../tools/trusted_gate/deploy/package-pilot.py), a recorded Core
release build and the committed source. The packager requires a clean tracked
checkout, matching Core crate tree and binary digest, and explicit release
incremental=false. It reads package/policy files from Git objects, embeds a
bounded Git bundle and produces `release.tar`, `install-pilot.py` and
`release.json`. It includes no live keys, tokens, raw evidence or handoff state.
The archive digest and exact source revision must match the reviewed publication.
The packager does not install anything or change GitHub.

The administrator copies that release and installer into a root-owned private
staging directory, checks their supplied SHA-256 digests, and runs the
[installer](../tools/trusted_gate/deploy/install-pilot.py) there with `--release`
and `--sha256`. This requires actual administrator authentication. The installer
refuses non-root invocation, non-private/unprotected archive paths, a mismatched
digest, unknown/missing/duplicate members, links, special files and oversized
payloads. It never extracts archive-selected paths into the host filesystem.
Installed destinations and delegated commands are fixed in the reviewed installer.

Existing differing installed files, unknown storage or active restricted-identity
processes stop installation; the installer does not silently overwrite/reclaim
them. A repeated identical release can resume its registered volume and retained
pilot state. Unknown running/failed Core invocations still require explicit
operator reconciliation, never another automatic full run.

## Administrator-selected Git proxy and preparation recovery

The protected Git environment deliberately ignores the developer's proxy environment
and global Git configuration. A host whose GitHub pack transfer needs a proxy must
configure that route explicitly in its root-owned private mirror. This is an
administrator network setting; the delegated launcher accepts no proxy option.
Keep Git's HTTPS certificate verification enabled. Scope the setting to GitHub:

```sh
sudo /usr/bin/git -C /srv/gatehouse/pilot/repository/mirror.git \
  config --local --replace-all 'http.https://github.com/.proxy' http://HOST:PORT
```

Use the administrator's approved proxy endpoint, without embedded credentials.
Do not import developer Git configuration or environment variables into the root
controller. API transport remains independently configured; successful API reads
or `ls-remote` alone do not prove that Git pack transfer works.

If the retained validation stderr shows a baseline `git fetch` timeout in
`source.prepare`, before any Core invocation was created, retain that log and
resume the **same reviewed release** after correcting this network route. The
freeze must still match the same head, base and bundle; changed input remains
blocked. Preparation failure does not authorize a quality PASS. If a Core run
already exists or its state is uncertain, reconcile it explicitly instead of
rerunning verification. Source preparation transport errors return a bounded
blocked result; transport output and credentials are not included in it.

## Upgrade the accepted fa18 pilot to the worker-history probe

The history candidate cannot be installed over an existing pilot with the normal
installer: differing protected files and a different release marker remain
blocked. The separate [upgrade-history-pilot.py](../tools/trusted_gate/deploy/upgrade-history-pilot.py)
entry supports only the accepted `fa18d094` transition release and its exact
archive/installer digests. The local packager supplies this entry beside the new
installer/archive, with all three review digests. Packaging still marks
`administrator_ready=false` until the exact-head candidate and deployment review
are complete. Do not execute a preflight archive as an upgrade.

After verifying and staging the reviewed files under a private root directory,
the administrator invokes the upgrade entry with the archive SHA-256 and the
separate installer SHA-256. Retain the original root staging directory
`/root/gatehouse-issue98-pilot-fa18d09`: it provides the verified old archive and
installer used by this one-time migration. No keys are included in either archive.

The entry holds the existing controller lock, checks restricted accounts/processes,
proves Core/worker units stopped with empty cgroups, verifies the old policy and
toolchain pins, and independently checks the repository/current base. It refuses
an attached/published pilot or another active candidate. It preserves the old
configuration, acceptance, volume marker and input bundle under
`/srv/gatehouse/pilot/control/history-upgrade-<new-release-digest>/`.

Before revoking or removing the old source, it copies and compares the complete
canonical publication and records its full file inventory. A partial prior copy
blocks removal. It then revokes the exact old transition run through the existing
controller, retires its source and retains the publication beneath that old run.
The previous negative Check and validation logs remain. Only the reviewed
history/source/isolation/probe file changes are allowed; Core/policy bytes,
credentials, App settings, required rules, volume UUID and existing mirror proxy
are preserved. New pins/metadata and the single `git_history` probe selection are
checked against an explicit expected configuration before it replaces the old one.

The protected upgrade intent records `prepared`, `retired` and `installed`
checkpoints. Repeating the same reviewed upgrade can resume known checkpoints;
unknown files, identity changes, partial evidence, a changed base or an unknown
invocation remain blocked with their recovery records. An already failed/running
new invocation requires explicit reconciliation. No retry implicitly reruns Core.

After releasing the installation lock, the new candidate enters through the same
developer public launcher. Its actual worker probe verifies the sanitized `.git`
pointer, read-only history/source, exact head/base refs, clean status, differences,
archive reads, fresh configuration and separate temporary fixture repository
identity. Existing key/control/socket/network denials still apply. The complete
pilot publication is checked by Core and retained; the separate negative App Check
still completes with failure and authoritative publication stays blocked. This
accepts an isolation/history prerequisite, not complete project/native coverage.

### Observed protected history acceptance

On 2026-10-17 the administrator executed the reviewed fixed upgrade to
`659803a2f144926997e56d072c7fe8760d6ed176`, archive SHA-256
`04190f57c76f9625be1e805ab212c48802181f458fecdc34f5478f796efd77ef`.
The bounded acceptance summary recorded run
`ce86a2a4-ff18-435f-bb5d-aafc313ceca6` and worker Git history passed within the
protected Core boundary step. The actual developer UID was 999, with only the
developer/submission groups; protected access and delegated-command denials held.

Independent GitHub readback confirmed negative Check `117423517761`, the exact
candidate SHA and isolation-pilot external identity, App `4867361`, and completed
`failure` as intended. Independent system journal reads showed the Core and both
workers deactivated successfully, including the actual `executor-history` probe;
no matching units remained active. The persistent volume used 32 MiB of 2 GiB.
Private publication and upgrade-retention files were not opened by the operator
Agent; those checks are reported by the pinned administrator entry, not claimed
as a second direct inspection. The previous fa18 evidence remains retained.

The installed release is now `659803a`, still transition-only with authoritative
publication blocked and production disabled. Do not use the ordinary installer
or the fa18-only upgrade for a later differing release. A later upgrade requires
its own reviewed reconciliation of the outstanding run and preservation of its
evidence. Full project/native coverage and restricted Agent operation remain
pending; this acceptance changes no required rule or workflow trigger.

## Exact host changes

- Install the reviewed Python package, public launcher, access probe and Core
  release executable under `/opt/gatehouse`, root owned and pinned.
- Copy the two existing operator-held signing keys into root-only `/etc/gatehouse`
  files. Do not change the existing Apps, unrelated consumers or operator signing
  copies. The installed client requests narrowly scoped, one-repository tokens.
- Create a fixed sparse 2-GiB ext4 image at `/var/lib/gatehouse/pilot.ext4` and
  enable only [srv-gatehouse-pilot.mount](../tools/trusted_gate/deploy/srv-gatehouse-pilot.mount)
  at `/srv/gatehouse/pilot`. No shared host filesystem mount options are changed.
  The image persists across restarts and bounds Git expansion, source, logs,
  reports, inbox, scratch and cache together. Directories remain separated and
  control/evidence are root private. Shared capacity can cause a blocked pilot;
  production retention/reservations remain part of the later acceptance.
- Create the bounded inbox `/srv/gatehouse/pilot/inbox`, root-owned sticky 1730
  with the submission group. Close the old empty `/var/spool/gatehouse/inbox` to
  root-only 0700 so it cannot become an unbounded alternate submission location.
- Install `/etc/gatehouse/host.json` as root-only 0600. Its configuration is
  **transition**, has the actual developer UID, exact protected-file pins and the
  four pilot step IDs. It cannot publish authoritative success.
- Validate and install only the existing narrow public-launcher sudoers rule.
  Do not grant a root shell, internal step, maintenance, arbitrary systemd or
  policy/configuration editing capability.

The reconciliation timer, production required rule and workflow triggers are not
enabled or changed. Future developer/repair/publishing Agent processes must run
under the restricted identity before production acceptance. A `gem` session still
belongs to the trusted operator domain, regardless of checkout location.

## Actual acceptance performed by the installer

1. Confirm the two accounts are distinct, have no privileged groups and have no
   active processes at installation. Confirm the submission group has exactly the
   intended writer and the worker has a non-login shell.
2. Independently authenticate both Apps and read GateHouse's numeric repository
   identity through narrowed read tokens. Never print credentials or API bodies.
3. Run the installed [probe](../tools/trusted_gate/deploy/pilot-probe.py) as the
   actual developer UID. It attempts protected-file access without reading bytes
   or truncating/writing files, tests management sockets and rejects arbitrary
   root/internal launcher operations.
4. As that developer, submit the exact packaged candidate through the public
   launcher. The root controller independently fetches the base, verifies the
   bundle and runs the existing Core with
   [the explicitly named pilot policy](../tools/trusted_gate/deploy/pilot.flow.toml).
   Core owns its two built-in gate results and the two external pilot steps.
   Each external step runs in the separate worker systemd sandbox.
5. The worker probe verifies its actual UID/groups, zero effective capabilities,
   protected key/state/policy/source denials, inaccessible management sockets and
   a network namespace containing only loopback. The host transport contract suite
   executes against the immutable candidate. Core retains the complete pilot
   publication and the host independently checks its artifact inventory/hashes.
6. Attempt authoritative publication through the real delegated entry. Transition
   coverage must reject it. Exercise a separately named **negative** App Check
   (`GateHouse Host Isolation Pilot (negative)`), complete it with **failure** and
   read back the dedicated App/SHA/run identity. A retry recovers that same Check.
   This intentionally failing, non-required probe is not a project quality result
   or an approval to merge.

The private acceptance record is
`/srv/gatehouse/pilot/control/pilot-acceptance.json`. Validation stdout/stderr and
Core logs are private and retained, including failed attempts. The installer prints
only the bounded acceptance summary. A failure leaves the slot/evidence for
diagnosis; it does not weaken policy, retry Core implicitly or migrate the rule.

The Core profile is `full` **within this explicitly limited pilot policy**. Its
coverage is not the complete GateHouse migration inventory. Complete native and
Linux execution adapters, actual restricted Agent operation, production cache and
retention budgets, stale/retry/merge negatives and the protected evidence endpoint
must still pass before production mode can be accepted. Retain the existing CI
owners in the [coverage inventory](trusted-host-coverage.md).

Before rollback, prove Core/worker cgroups stopped and reconcile any outstanding
publication. Disable the delegated entry and pilot mount only after preserving
private evidence and confirming the source slot is terminal. Do not remove the
image, keys, journal or existing accounts as an automatic cleanup action.
