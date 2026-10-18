# Gate results and external consumers

GateHouse accepts candidate input and policy, runs the existing Rust Core plan,
and returns its quality conclusion, reasons, coverage and evidence. The local
machine result and exit code are independently usable. A result does not need a
PR, GitHub Check, queue, publisher service or merged commit to be complete.

## Ownership

| Owner | Responsibility |
| --- | --- |
| Rust Core (`tools/harness-gate`, `quality-core`) | Flow, applicable checks, requiredness, coverage, thresholds, baseline/debt/ratchet, waivers, evidence verification and final quality decision |
| Execution environment | Reviewed programs/tools, identity, process containment, protected policy/evidence access and execution resources |
| [GateProbes](https://github.com/musutrade/GateProbes) | Language/tool measurements, collectors, presets, plugin tests and independent fixed-artifact releases |
| Result consumers | Delivery credentials, platform API behavior, delivery records, PRs, CI/merge policy, task queues and agent coordination |

Measurement plugins return facts; they cannot select a reduced set of mandatory
checks or declare project PASS. A fixture can replace a measurement process but
must still use the real Core decision. Core builds and tests do not require a
GateProbes source checkout. Compatibility uses fixed plugin identities, not
`latest` or an implicit rebuild of plugin source in Core CI.

One execution may acquire resource leases and handle timeout, cancellation,
process termination and cleanup. Those leases concern actual execution resources;
they are not publication slots and cannot depend on PR/merge status. A leaked
process, invalid evidence or unmet required check cannot produce a valid PASS.
Local process tests do not establish production OS isolation.

## Existing result contracts

Keep the machine result, invocation metadata, artifact registry and manifest as
the result/evidence contract. Do not convert an agent's summary or a transport
success into a quality conclusion. [Directory inputs](input-snapshots.md) bind
the supported normalized candidate files; [saved-result verification](saved-results.md)
checks pinned historical evidence independently of its original source path.
Current applicability requires candidate, baseline, policy, Core/plugin/tool and
execution-environment bindings. Historical integrity alone does not establish it.

Automatic Webhook delivery was removed from `verify`: consumer availability no
longer changes verification completion or the quality exit status. Legacy
notification configuration emits a migration warning. External consumers own
their own credentials, retry/deduplication and delivery status. They must leave
the saved result unchanged and must not map missing/invalid evidence to success.

## Excluding draft PR #127

The implementation baseline is `0d969e03ab2a58591eb39ca67d7dba4d5cf04581`.
Draft [PR #127](https://github.com/musutrade/GateHouse/pull/127) is reference
material, not a required merge. Its `tools/trusted_gate` controller, publication
state, source retirement, installer/recovery and cache manager are absent from
that baseline and are not imported by this work.

For #137, do not add `merged`/`revoked` admission conditions, batch membership,
publication slots, merge observation or a replacement GateHouse scheduler.
For #138, do not add `cache.py`, cache compatibility keys, capacity/TTL/GC,
`maintain-cache`, dedicated cache-volume requirements or their installation
records. No local native cache configuration or data is changed. Execution
accounts may use protected Cargo/sccache/package-manager/framework configuration;
cache hits do not prove that checks ran successfully. The existing opt-in step
reuse delivered under #32 is a separate feature and remains intact.

These exclusions describe the main-based source boundary. They do not claim that
an already installed historical host has been removed, repaired or upgraded.
The existing incident and all side effects remain under the independent
[operator recovery runbook](operator-recovery.md).

## Historical host records

Preserve original host journals, evidence and remote delivery records. Do not
rewrite their old verdicts, silently convert them into current results or copy
PR/Check/merge state into a new invocation. Preserve unknown and failed states as
such. A historical result can be imported for reading only when its complete
original evidence and independently trusted bindings satisfy the existing
verifier; missing bindings cannot be repaired by relabeling it PASS.

New invocations write ordinary Core evidence independently of old publication
state. Source cleanup and evidence retention require stopped execution and safe
evidence storage; neither waits for PR merge. Operational migration of a live
host is a separate authorized operation and is not performed by reading or
validating a result.

## Acceptance limits

The directory-input regression uses real Core execution and denies Git/platform
commands; the saved-result regression removes its source before reading and then
tests evidence tampering. Consumer separation tests retain both PASS and FAIL
without contacting a configured endpoint. These are local contract tests.

This document does not certify full Linux Test workload coverage, production
isolation, completed host recovery, a new plugin release or a GitHub Check
consumer. Those claims require their own exact inputs and evidence. In particular,
#139's complete policy/runtime applicability work and #131's optional platform
consumer remain independently tracked; the architecture umbrella must not be
closed merely because these initial local paths exist.
