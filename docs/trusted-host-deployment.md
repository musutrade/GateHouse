# Trusted host deployment on the existing Linux machine

This is the selected deployment plan for [#98](https://github.com/musutrade/GateHouse/issues/98).
It is a staged installation plan, not a claim that production acceptance passed.
The [coverage inventory](trusted-host-coverage.md) still has unassigned cells.

## Identities and administrator prerequisite

Keep `gem` as the trusted operator. Run future development, repair, publication
Agent sessions and their editors/processes as `gatehouse-developer`, with no
membership in the operator's groups or access to operator sessions, credentials,
SSH agents, containers or system-management sockets. Run untrusted build/test
commands as the separate non-login `gatehouse-executor`. Run the installed Core,
controller, report writer and App client in the protected root control domain.
An Agent still running as `gem` remains inside the operator trust domain; moving
only its checkout does not establish acceptance.

The [identity provisioner](../tools/trusted_gate/deploy/provision-identities)
creates only the two accounts, a submission group and empty fixed directories.
It refuses pre-existing managed identities/locations and unsafe parents before
mutation, never changes `gem`, and never installs keys, sudoers, policy or services.
It requires a reviewed root invocation. If it stops partway through, reconcile
the created accounts/directories explicitly; rerunning never silently takes over
an existing installation.

At the 2026-10-17 inspection, `sudo -n true` could not authenticate. The Docker/LXD
capabilities of `gem` are not an alternate administrator-authentication mechanism
for this deployment. An administrator must authenticate in their own terminal;
do not send passwords, App keys or tokens through chat.

After that initialization and App repository authorization, use the separately
reviewed [bounded isolation pilot](trusted-host-pilot.md) to prepare the complete
installer and run actual identity probes. The initial provisioner alone does not
install the package, keys, policy, delegated entry or service.

Inspect and copy the accepted release into a root-owned staging directory first.
Run the reviewed provisioner from that snapshot with a clean environment:

```sh
/usr/bin/env -i PATH=/usr/sbin:/usr/bin:/sbin:/bin \
  /bin/bash /root/gatehouse-release/tools/trusted_gate/deploy/provision-identities --apply
```

Provision filesystem quotas for scratch/cache/inbox and retain policy-controlled
budgets. The provisioner deliberately does not guess the filesystem's quota
configuration or alter a shared mount. Set ownership/modes and verify ACLs as well
as ordinary permissions. Keep operator home traversal unavailable to both new
identities. Existing signing copies may remain only within the trusted operator
domain; rotate/revoke any copy exposed to an untrusted identity or Agent session.

## Protected installation and narrow submission entry

From the accepted root-owned release snapshot, install:

| Source | Fixed installed destination | Ownership / mode |
| --- | --- | --- |
| `tools/trusted_gate/*.py` | `/opt/gatehouse/trusted_gate/` | root:root; directories 0755, files 0644 |
| Reviewed Core executable | `/opt/gatehouse/bin/harness-gate` | root:root 0755 |
| Reviewed complete flow and `deploy/gatehouse-step` | `/etc/gatehouse/policy/` | root:root; directories 0755, files 0644, launcher 0755 |
| `deploy/gatehouse-control` | `/opt/gatehouse/bin/gatehouse-control` | root:root 0755 |
| Completed `deploy/host.example.json` | `/etc/gatehouse/host.json` | root:root 0600 |
| Protected App signing material | `/etc/gatehouse/checks.private-key.pem`, `/etc/gatehouse/delivery.private-key.pem` | root:root 0600 |

Initialize the root-owned Git mirror in `/var/lib/gatehouse/mirror.git` using the
fixed repository without importing Agent Git configuration. Populate reviewed
toolchains/dependencies in their protected paths. Set `developer_uid` to the
actual newly provisioned account; the example's zero placeholder is deliberately invalid.
Fill the complete protected-file pins, step inventory, exact commands, toolchain
identities and environment. Keep `coverage_acceptance: transition` until all
required acceptance has passed. Do not publish the configuration or signing files.

The public launcher is eligible for only the
[specific sudoers entry](../tools/trusted_gate/deploy/sudoers.example). Validate
the installed rule with `visudo -cf` before enabling it. It permits the fixed
launcher, whose only actions are `validate`, `deliver`, `attach`, `publish`,
`status` and `reconcile`. Python runs with `-I`, a fixed installed module path,
and a fresh environment. Internal `step`, maintenance, revocation, retirement,
arbitrary systemd units, interpreters and shells are not delegated. Configuration
is always `/etc/gatehouse/host.json`; callers cannot select another one.

The root-owned inbox is `/var/spool/gatehouse/inbox`, group `gatehouse-submit`,
mode 1730. Only single files there, owned by the configured developer UID with a
single hardlink, can supply a bundle or PR body. Reads use one no-follow file
descriptor and a size limit. A PR body path cannot turn into a read of a root key.
The inbox is disjoint from source, policy, control state, mirror, scratch and cache.

As the restricted developer, create a bundle and body file in that inbox, then
use the fixed launcher. Obtain the run ID from its structured result before any
next action:

```sh
git bundle create /var/spool/gatehouse/inbox/candidate.bundle HEAD
sudo /opt/gatehouse/bin/gatehouse-control validate \
  --bundle /var/spool/gatehouse/inbox/candidate.bundle --head FULL_REVIEWED_SHA
sudo /opt/gatehouse/bin/gatehouse-control status
```

Do not install the reconciliation timer until its installed command/configuration
and access have passed the same review. Maintain an operator recovery route for
unknown running cgroups, failed remote revocations and terminal source retirement.

## Acceptance and activation

First run actual developer/worker probes: neither may read signing keys/private
state or modify configuration, installed code, journal or evidence; neither may gain an
operator shell or use Docker/LXD/systemd management. Probe the developer's exact
sudo command inventory and worker mount/process/network view. Exercise malicious
body/bundle paths and code attempting to alter trusted source or forge reports.
Record actual denials and corresponding blocking Check outcomes. Unit-test mocks
of ownership or an administrator running these probes are not deployed proof.

Add GateHouse to the existing delivery App installation through the authorized
App administrator. Keep unrelated repositories/consumers unchanged. Use narrowly
requested one-repository installation tokens and prove the actual signing keys
are inaccessible from both restricted identities. Existing installation discovery
does not establish that delivery authorization or identity separation is active.

Complete every applicable row in the coverage inventory, real Rust/frontend cache
measurements and the crash/retry/stale-head/base/policy/evidence/epoch/merge
negatives in [the host acceptance list](trusted-host.md#persistent-cache-and-acceptance).
Provision the existing protected evidence endpoint. Perform a pilot with real
Core evidence and real dedicated-App Checks, including failed and revoked states.
Then review the exact protected configuration and accepted evidence before
changing the required rule or any existing validation trigger. Native Actions
remain required until equivalent native execution is accepted.

Rollback leaves the current required aggregate/workflows in force, stops new
delegated submissions, revokes any inapplicable host Check and retains the journal
and evidence for operator recovery. Never remove an active publication record or
free its slot merely because a caller or transport timed out.
