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
