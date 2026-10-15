# Core internal API contracts

This guide is for maintainers changing configuration loading, internal commands,
resource ownership or verification output. The names below are existing Rust
interfaces, including crate-private functions; they are not a new public SDK.
[Engineering Policy](engineering-policy.md), measurement identities, required
gates and failure semantics are unchanged. Production `unsafe` preconditions
remain documented at their implementation sites (issue #57).

## Contract gaps addressed

| Boundary | Caller question this guide resolves |
| --- | --- |
| Configuration | Does parsing a string avoid environment reads or enforce the disk input limit? |
| Process capture | Does `Ok` mean command success, and may cleanup ignore cancellation? |
| Service lease | Does releasing a lease stop its resource, and can `Drop` report failure? |
| Verification report | Does writing JSON certify complete evidence or publish all files atomically? |

These are documentation gaps, not claims of newly discovered runtime defects.

## Configuration: loading is more than TOML parsing

[`FlowConfig::load_with_diagnostics` and `from_source_with_diagnostics`](../tools/harness-gate/src/config/loader.rs)
are used by project discovery and quality configuration. Disk loading first reads
UTF-8 with a 4 MiB limit, checking both initial metadata and the bounded read so
file growth cannot bypass the limit. Read errors become `ConfigDiagnostics`.
The in-memory entry accepts an already allocated string; it does **not** apply
that disk limit. Callers accepting untrusted strings must impose their own input
budget before calling it.

The shared pipeline rejects removed external-service configuration and forbidden
audit-config interpolation, interpolates environment values, deserializes TOML,
applies environment overrides, then validates the resulting configuration.
`from_source` wraps the same pipeline in `anyhow::Result`; it is not an isolated
syntax parser. Reproducible tests must control relevant environment variables.
A successful return supplies a validated `FlowConfig`; malformed input or invalid
overrides return diagnostics, not a partially accepted configuration.

`source_path` supplies diagnostic provenance. `repository_root` enables the
filesystem-aware report-template checks in
[`validate_with_diagnostics`](../tools/harness-gate/src/config/validation/mod.rs).
Passing `None` does not establish that template paths exist or remain inside a
particular checkout. Project discovery passes its execution root. Callers that
construct `Project` directly still encounter the report writer's runtime path
checks; configuration validation is not a replacement for those checks.

## Process capture: execution status and transport failure differ

[`capture`](../tools/harness-gate/src/process/capture.rs) starts `program` directly
with separate arguments in `cwd`; it does not interpret a shell command string.
It returns raw stdout/stderr bytes and `ExitStatus`. A nonzero child exit can
still produce `Ok(CapturedOutput)`: the caller must interpret `status.success()`
and the command's own output contract. Captured bytes are not automatically
UTF-8 or redacted; callers must apply the appropriate decoding and disclosure
policy before displaying or publishing them.

`capture` observes global cancellation. `capture_cleanup` deliberately does not,
so teardown can attempt to stop resources after cancellation; it still has a
command timeout and output/reader limits. The runtime adapter uses this cleanup
entry. `capture_environment`, used by recipes, clears the inherited environment,
sets the supplied map and closes stdin. Ordinary capture inherits the default
command environment and stdin behavior.

Stdout and stderr are drained concurrently with separate byte limits. Overflow,
cancellation and command timeout terminate the child through the process helper;
spawn, wait, reader and termination errors propagate. Overflow is an error,
not successful truncated output. The combined byte limit is also checked after
collection. Child exit is checked before cancellation and timeout in the polling
loop, so a completed child can win that race.

The command clock starts after spawn and reader setup. After the process exits
or is terminated, both readers share a separate reader deadline. Termination and
reader collection can therefore extend total elapsed time beyond the command
timeout; this parameter is not an end-to-end wall-clock guarantee. The short
polling backoff is an implementation detail, not a promised minimum latency.

## Service leases: ownership precedes destructive cleanup

[`ResourceLease`](../tools/harness-gate/src/service/lease.rs) records project,
resource, invocation, PID and process-start identity. Acquisition creates an
ownership marker and starts a heartbeat; it rejects unprovable process identity.
An existing record must be validated before stale-resource reclamation. A live
PID with unknown identity is not permission to reclaim. Report-directory markers
have a stronger rule: generic stale expiry cannot silently replace them; they
require explicit proven cleanup and a report-directory guard.

A container's immutable runtime object ID is bound after inspection through
`bind_runtime_identity`; repository configuration cannot substitute that identity.
[`RunningService::drop`](../tools/harness-gate/src/service/mod.rs) verifies runtime
ownership, attempts the runtime stop, then calls `release_checked`. If stopping
fails, it retains the lease marker and records cleanup failure. **Releasing a
lease alone does not stop the runtime resource.** Keep this ordering when
changing adapters or moving container-specific code out of Core.

`release_checked(self)` stops and joins the heartbeat before releasing ownership,
returns failures, and suppresses another release attempt on drop. Heartbeat
failure blocks release and ownership verification. `retain(self)` stops renewal
and leaves the marker for inspection. Plain `Drop` attempts release but cannot
return its error; use checked release when a caller must report the outcome.

For report directories, checked release writes the release certificate while the
marker still protects the directory, then removes the marker. The directory
guard remains held through that sequence. A missing report marker is an error;
ordinary lease release tolerates a missing marker. Do not generalize the latter
behavior to report-directory completion or treat expiry as proof of completion.

## Verification reports: completion is an evidence transition

[`publish_report`](../tools/harness-gate/src/verify/mod.rs) calls the report writer,
quality publication, compatibility mirroring and notification in that order.
Each stage can fail. Earlier files can remain when a later stage fails; this is
not one filesystem transaction. The invocation-specific directory is allocated
with a lease by [`allocate_invocation`](../tools/harness-gate/src/verify/report.rs).
Compatibility copies do not replace the invocation's bound evidence.

`report::write` first redacts invocation content and renders documents, including
a preliminary machine result explicitly marked as incomplete. It then assesses
the declared evidence set. Successful finalization writes registry and manifest,
verifies them, writes the final machine result, and verifies the combined result.
This ordering prevents an interrupted control-file sequence from being presented
as complete merely because a JSON file exists. Document-render and control-file finalization failures attempt to leave an
incomplete machine result and propagate failure. Earlier errors can return
before that fallback. Retention pruning runs after finalization and can fail
while leaving complete evidence, as can the later quality publication, mirroring
and notification stages. A completed evidence document is therefore not proof
that the entire publication call succeeded.

`Ok(())` from `write` is not itself a PASS decision: when the verification already
failed, an incomplete evidence assessment can be recorded without returning a
second error. Callers and consumers must retain the report outcome and evidence
completion state. Likewise `VerificationReport` serialization alone does not
perform filesystem evidence finalization.

Output paths must be report-relative. Runtime checks reject traversal and
pre-existing symlink escapes, including when callers bypass configuration
loading. Final writes use
[`confined_atomic_write`](../tools/harness-gate/src/utils/fs.rs); retain that
primitive rather than replacing it with a check followed by an unrestricted
write. Atomicity of an individual file is not atomicity of the report set.

Machine-result and generated quality-report JSON pass through structural
redaction so numeric evidence and JSON shape are preserved. Generated files bypass the general text-redaction pass only when
their exact bytes match the hash recorded by the current writer; a filename or
extension alone cannot grant this generated-JSON exemption. The traversal also
skips files named `manifest.json` and files ending in `.tmp` after its size
checks. Other regular evidence files use bounded text redaction, including lossy
decoding for invalid UTF-8. Exceeding size budgets fails publication instead of
skipping redaction. The redaction traversal's
handling of non-regular entries is not an acceptance decision: the subsequent
evidence assessment and manifest validation still govern publication.

## Checking documentation changes

Use the existing contribution workflow, including fixed-digest regression data:

```sh
python3 tools/maintenance/test_data.py
python3 tools/quality/docs_consistency.py --pure --output target/quality/docs-consistency.json
```

These commands validate documentation contracts; they do not certify runtime
behavior. Changes to the implementations above also require their relevant
regressions and normal required quality gates. See [Contributing](../CONTRIBUTING.md)
and [CI operating guide](quality/ci-topology/README.md).
