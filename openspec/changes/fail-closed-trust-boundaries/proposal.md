# Proposal: Fail-Closed Trust Boundaries

**Status:** Implemented
**Date:** 2026-08-31
**Change type:** Invocation input, resource ownership, evidence, filesystem, and release integrity

## Goals

- Make every verification node read one immutable invocation input.
- Require runtime identity and complete ownership metadata before destructive cleanup.
- Treat reported, manifested, and on-disk evidence as one closed set.
- Publish every predictable Harness-Gate output through one confined, atomic writer.
- Derive checksum, signature, provenance, and upload operations from one explicit release inventory.

## Non-goals

- Do not add an operating-system sandbox to the adapter protocol.
- Do not accept untrusted adapter requests or change the adapter protocol version.
- Do not broaden this change to webhook policy, installer verification, parser strictness,
  performance tuning, or general repository cleanup.
- Do not mark ADR-0034 accepted before all Phase 0 acceptance evidence is reviewable.

## Success Metrics

| Boundary | Success criterion |
| --- | --- |
| Invocation input | Opposing index/working-tree fixtures prove every hook node observes only the staged snapshot. |
| Runtime ownership | Forged, renamed, cross-project, malformed, or identity-mismatched leases never invoke runtime removal. |
| Evidence | Missing, escaped, replaced, stale, or undeclared artifacts make publication fail with `evidence_complete = false`. |
| Filesystem | Predictable outputs reject target and parent symlinks and appear atomically on supported platforms. |
| Release | The verified checksum/signature/provenance subject set exactly equals the uploaded asset inventory. |

## Risk Assessment

**Risk: High.** These changes alter the source tree presented to commands,
destructive resource cleanup authorization, successful-result publication, and
release creation. Failures must retain evidence and stop the operation rather
than silently falling back to the prior behavior.

The implementation is split by boundary, with focused failure fixtures before
each shared abstraction is adopted. Compatibility is preserved for non-hook
working-tree invocations and for correctly owned resources and complete
evidence. A release remains blocked whenever inventory verification is
unavailable.

## Delivery Sequence

1. Introduce the invocation input descriptor and staged execution root.
2. Bind resource leases to project, invocation, labels, and immutable runtime identity.
3. Introduce the confined output publisher and migrate predictable outputs.
4. Add the invocation artifact registry and closed-set finalization.
5. Replace release globs with one generated and verified asset inventory.
6. Run the complete local and cross-platform acceptance suite, then update statuses.

## Related Records

- [ADR-0034: Enforce Fail-Closed Trust Boundaries](../../../docs/adr/0034-fail-closed-trust-boundaries.md)
- [ADR-0031: Harden Gate Boundaries and Delivery Contracts](../../../docs/adr/0031-harden-gate-boundaries.md)
- [ADR-0032: Harness-Gate and DevRail Capability Contracts](../../../docs/adr/0032-harness-gate-devrail-capability-contracts.md)
- [ADR-0033: Signed Out-of-Process Adapter Protocol](../../../docs/adr/0033-signed-out-of-process-adapter-protocol.md)
