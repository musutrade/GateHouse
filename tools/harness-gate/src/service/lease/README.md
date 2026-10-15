# Lease ownership boundaries

Resource leases protect both containers and report directories. Their marker,
heartbeat and process identity semantics remain Core responsibilities. The
private `container` child module groups container-specific ownership checks and
the existing runtime adapter; it is not an external plugin or a new protocol.

| Responsibility | Owner and callers |
| --- | --- |
| Marker format, atomic persistence and validation | `lease.rs`; acquisition, renewal and cleanup |
| PID/start identity, heartbeat health, checked release and retained failures | `lease.rs`; `ResourceLease` and service teardown |
| Report-directory lock and completion certificate coordination | `lease.rs` with `report_directory.rs`; invocation publication, retention and explicit cleanup |
| Reclamation selection, dry-run reporting and marker removal | `lease.rs`; operator cleanup and stale-lease acquisition |
| Container name/label/immutable-ID checks | `lease/container.rs`; binding, verified teardown target and stale-container reclamation |
| Runtime inspect/stop abstraction | `lease/container.rs`; delegates actual commands to `service/runtime.rs` |
| CLI argument construction and bounded execution | Existing `service/commands.rs` and `service/runtime.rs` |
| Ownership-label construction | `lease.rs`; generic record creation/validation, container validation and `service/docker.rs` |

The dependency direction is:

```text
ResourceLease / cleanup
  -> container identity verification
     -> RuntimeOperations -> service/runtime.rs -> bounded process capture
  -> stop the verified immutable object ID (for container reclamation)
  -> remove the ownership marker only after successful cleanup
```

`ResourceLease` itself does not stop a container when its marker is released.
Normal service teardown verifies ownership and stops the verified ID before
checked release. Reclamation likewise leaves the marker when runtime cleanup
fails. Heartbeat errors, unknown process identity, changed runtime identity and
mismatched ownership labels remain failures. Report-directory cleanup retains
its separate guard and proof-of-owner-exit rules; it does not become container
cleanup or acquire an expiry-only fallback.

This boundary addresses a maintenance coupling: container inspection and runtime
calls previously sat beside generic marker persistence and report-directory
protection. Moving those helpers makes the runtime dependency explicit without
moving final cleanup decisions or changing release order. Label construction
stays in the parent because even generic lease creation and validation use it.
Tests remain at their existing locations and exercise the child through private
parent imports. Test-only process-observation overrides and platform-specific
process identity code remain in `lease.rs`.

The preceding cleanup-identity fix is a separate behavior change. This extraction
preserves that fix and the existing interfaces seen by other Core modules. It
makes no complexity, speed or removed-production-line claim.

Both the blocking `service-core` production inventory and the certified
`SOURCE_FILES` include the child source. Base/head measurement uses the same
inventory, recording the new file as absent at base. Moved function identities
must pass normal changed-function gates; historical debt is not transferred.
The identity-mismatch critical-path binding follows `verify_runtime_record` to
the child while its regression test stays in the parent. Probe and assertion
semantics are unchanged, as are Engineering Policy and measurement definitions.

Issue #62 still governs any future GateProbes runtime migration. The existing
`gate-service-check/v1` check result does not establish start/stop, persisted
ownership, heartbeat or crash-recovery protocol support. This internal split
neither claims that support nor changes configuration, installation or plugin
compatibility.
