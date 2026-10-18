# Independent operator recovery

This runbook addresses the historical host incident tracked by
[GateHouse #135](https://github.com/musutrade/GateHouse/issues/135). It does not
install a host, authorize a recovery attempt, or add a deployment requirement to
Core verification. The implementation in draft PR #127 is reference material;
it must not be merged as a prerequisite for this procedure or normal gate use.

GateHouse completes an invocation when it produces its quality result and
evidence. Host recovery is a separate operation with its own authorization,
identity and records. An unavailable trusted execution environment cannot claim
a valid result, but an unrelated failed recovery does not rewrite a previously
completed result or place it into a publication queue.

## Historical incident, not current status

The incident involved interrupted installation `063eac64`, a 16 GiB loop image
and a 28-file installation plan. A recorded attempt ended after 158.4 seconds
with `status=blocked`, exit code 1, files 0/28 and a failed service. These values
describe that attempt, not the current host. They are not generic capacity,
installation-size or performance requirements.

The retained launcher was
`/mnt/dev-ssd/gatehouse-issue98-operator-review-observation/start-detached-linux.py`.
It stages private programs under `/root`, writes operation output under
`/var/lib/gatehouse`, and dispatches a detached systemd service. Its 30-second
dispatch timeout does not limit or prove completion of the detached work.

For that historical launcher, the observation addresses are:

| Record | Historical address |
| --- | --- |
| Unit | `gatehouse-issue98-recovery-063eac64c53b.service` |
| Public progress | `/run/gatehouse-issue98-upgrade-063eac64c53b.json` |
| Private result | `/var/lib/gatehouse/operator-runs/063eac64c53b/result.json` |
| Private logs | `stdout.log` and `stderr.log` beside the private result |
| Private staged programs | `/root/gatehouse-issue98-detached-063eac64c53b` |

Read the reviewed launcher and operation plan to establish addresses for any new
attempt. Do not assume a later candidate uses these names or paths. Keep private
logs, configuration, credentials and raw host evidence out of GitHub.

## Prepare and authenticate

1. Review the exact candidate, its source commit and program digests, dependency
   pins, fixture results and applicable exact-candidate CI. Passing fixtures
   establish contract behavior only; they do not establish real host isolation,
   completed recovery or production readiness.
2. Inspect the current host and any prior operation read-only. Record current
   units/processes, installation receipts, interrupted transaction, execution
   identity, protected paths and environment/configuration identity. A PID alone
   is not a process identity; include start time and service ownership. Unknown,
   active or contradictory state blocks a new dispatch.
3. Prepare an operation plan identifying exact effects, inputs, retained prior
   evidence, observation addresses, stop conditions and the authorized operator.
   A new attempt must not overwrite the prior attempt or silently relax its
   integrity or isolation checks.
4. Obtain authorization for that exact operation through the environment's
   operator process. Authenticate interactively with `sudo -v` if needed. Do not
   put passwords into scripts, logs or agent messages. Documentation edits and
   an authorization for an earlier failed attempt do not authorize a new run.

## Start once and observe

The operator executes only the command recorded in the approved plan, once.
Do not automatically run the historical launcher above. A launcher success
means dispatch was confirmed, not that recovery succeeded. A timeout or lost
response means dispatch is uncertain: inspect the existing unit before doing
anything else. The historical launcher reports an existing unit without
dispatching a second operation; that report is not recovery success.

For the historical unit, these read-only commands illustrate the observations:

```sh
systemctl show gatehouse-issue98-recovery-063eac64c53b.service \
  --property=Id,LoadState,ActiveState,SubState,Result,ExecMainStatus
journalctl --unit=gatehouse-issue98-recovery-063eac64c53b.service --no-pager
```

Use the approved access method to read public progress and private result/logs.
Some environments require operator privileges for those reads. Correlate the
service, operation ID, candidate, execution identity, timestamps and receipts.
Check that execution has stopped before treating a result as terminal. Absence
of an error, a stale progress file or exit code 0 alone does not prove installation
or isolation acceptance. Confirm the plan's explicit postconditions and retain
their evidence.

## Failure, retry and archival

On failure, retain the unit status, private programs, output, receipts and partial
installation. Record the exact failing phase and reason. Do not reset the unit,
delete files, clean volumes/caches, unblock entrypoints or automatically rerun.
Separate diagnosis from repair: a read-only diagnosis must not become an implicit
installation attempt.

A retry needs a reviewed cause and correction, verified stopped prior execution,
fresh applicability checks against current host state, a new operation identity
and explicit authorization. Keep the original attempt's evidence. If a command
was already dispatched, reconcile its state before considering any retry.

After the operator independently verifies resolution, archive the one-time
scripts and necessary audit records under the environment's retention policy.
Record where the evidence is retained, who accepted the postconditions and which
temporary artifacts were retired. Archival is not an instruction to delete the
current incident directory or modify existing cache data.

Each operation record should include:

| Field | Meaning |
| --- | --- |
| `operation_id`, prior operation | Unique attempt and retained predecessor |
| candidate, program digests | Exact reviewed source and executed programs |
| environment/configuration identity | Host and protected configuration used |
| started/finished, elapsed | Observed time, distinguishing dispatch and execution |
| phase, unit, execution identity | Correlated progress and actual process/service |
| exit code, status, failure reason | Terminal observation, without promoting failure |
| progress/result/log locations | Retained evidence references and access policy |
| postconditions and acceptance | What was actually verified and by whom |
| authorization | Approval for this specific operation |

## Daily verification boundary

Normal Core tests and local result validation must not call recovery launchers,
create real large images, require Root/systemd, or modify installed hosts. Tests
of operation contracts may use isolated fake processes/filesystems and must
identify themselves as fixtures. Such tests do not start a production installer.

Distinguish four claims: fixture behavior, exact-candidate CI, live environment
acceptance, and production readiness. Evidence for one does not imply the others.
This runbook supplies the documentation required by #135; it makes no claim that
the historical recovery has succeeded and leaves live operations to their owner.
