# Adapter Isolation Wording Inventory

**Status:** Current contract inventory for R-07 (2026-09-03)

Every Harness-Gate adapter isolation claim is classified below as a
**protocol control**, **lifecycle control**, or **evidence control**. No
current record claims operating-system-enforced sandboxing, complete
descendant containment, or an OS network/filesystem/resource/process sandbox.
The future platform-sandbox decision is tracked separately in
[`os-sandbox-decision-matrix.md`](os-sandbox-decision-matrix.md).

## Inventory

| Record | Claim / wording | Classification |
| --- | --- | --- |
| [README.md](../README.md) "Adapter integration" | v2 host validates signature/digest/nonce, clears inherited environment, applies declared capability allowlist, bounds streams/artifacts, and attempts bounded process-tree cleanup on timeout/cancellation. Capability allowlist is a protocol-level declaration check, not an operating-system network/filesystem/resource/process sandbox; cleanup is not proof of complete descendant containment. | Protocol control; lifecycle cleanup; evidence control |
| [README.zh-CN.md](../README.zh-CN.md) | Chinese equivalent of the above bounded wording. | Protocol control; lifecycle cleanup; evidence control |
| [configuration.md](configuration.md) § adapter run | Host verifies versions, digest, signature, nonce/expiry, artifact confinement, clears environment, and injects only declared values. "This is a protocol-level boundary, not an operating-system sandbox." | Protocol control |
| [configuration.zh-CN.md](configuration.zh-CN.md) § adapter | Capability allowlist is a protocol failure boundary, not OS-level network/file/resource/process sandboxing. | Protocol control |
| [ADR-0033](adr/0033-signed-out-of-process-adapter-protocol.md) Security Boundary | Adapters run with per-invocation cwd, no inherited secret environment, and only declared protocol-level permissions. Checks are not an operating-system sandbox; process-group termination is a bounded cleanup attempt. | Protocol control; lifecycle cleanup |
| [ADR-0037](adr/0037-adapter-request-integrity-and-evidence-budgets.md) Decision | Capability allowlists remain protocol declarations; process groups and cleanup are not an OS/complete-descendant sandbox. | Protocol control; lifecycle cleanup |
| [ADR-0038](adr/0038-post-remediation-hardening.md) § 1 | Documentation and adapter contracts describe allowlists, cleanup, bounded readers, and protocol checks as protocol/lifecycle controls only; no OS-sandbox or complete-descendant claim. | Protocol control; lifecycle control |
| OpenSpec `harness-gate-adapter-protocol` design | Capability declarations and process groups are protocol/cleanup controls, not an operating-system sandbox. | Protocol control; lifecycle cleanup |
| OpenSpec `harness-gate-adapter-protocol` spec | Failure isolation limits crashes to one adapter node; it does not assert host isolation. | Evidence control |
| OpenSpec `adapter-request-integrity-and-evidence-budgets` design | Protocol-level allowlists and bounded cleanup attempts do not promise OS-enforced network/filesystem/resource/process isolation until a future sandbox design exists. | Protocol control; lifecycle cleanup |
| OpenSpec `post-remediation-hardening` spec | Adapter isolation claims SHALL name protocol validation, allowlists, bounded readers, and process-group cleanup; they SHALL NOT claim OS network/filesystem/resource or complete descendant isolation. | Protocol control; lifecycle control; evidence control |
| CLI `harness-gate adapter run` help | Describes signed request validation and repeatable `--allow-network` / `--allow-resource` / `--allow-environment` capability switches. No sandbox wording appears in help. | Protocol control |
| Source `process/adapter.rs` module docs | `run` executes one signed request in an independent process group with bounded host-side cleanup and result/evidence validation. | Lifecycle control; evidence control |

## Verification

The documentation-consistency gate runs `sandbox_wording` over README files,
`docs/**/*.md`, current OpenSpec change records, schemas, and Rust source
comments. It fails when a sentence makes an unsupported positive sandbox or
complete-descendant-isolation claim and passes when the wording above is
preserved. See [`tools/quality/docs_consistency.py`](../tools/quality/docs_consistency.py).

## Future boundary

An OS-enforced sandbox is not implemented by this inventory. Before any
record may advertise one, the separate platform decision matrix and
platform-specific evidence must be accepted; until then the bounded wording in
this inventory remains the compatibility contract.
