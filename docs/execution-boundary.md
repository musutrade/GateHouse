# Local execution and host responsibilities

The existing Rust Core owns workflow selection, required checks, quality
evaluation and local results. Its process layer runs the selected commands and
signed collectors; it does not install a host, configure system services,
allocate a publication slot or observe a PR. Local fixtures need none of those
services. Fixtures demonstrate the process contract, not production isolation.

On Unix, each command starts in a new session/process group. Timeout,
cancellation and output-limit paths terminate that group with bounded cleanup.
Normal leader exit also triggers group cleanup before the leader is reaped.
Keeping the leader waitable protects its PID/group identity from reuse during
cleanup. The original leader exit status is retained; a cleanup error is an
execution error and cannot become a successful task or collector result.
Normal completion uses no termination grace period: a command that exits has
finished its declared work, so background descendants cannot continue that work.

This is process-group cleanup, not a sandbox. A process that deliberately starts
another session may escape the group. Sending a signal does not attest that all
external effects have stopped. The portable non-Unix normal-completion path
still observes the direct child only; it provides no process-tree completion
guarantee. Production consumers requiring stronger containment must establish
it independently and must not treat an unverified boundary as satisfied.

Signed collectors clear inherited environment and validate their executable,
signature, capabilities, paths and host limits under the existing adapter
contract. Ordinary workflow commands retain their existing environment contract.
Run result consumers under a separate environment/account: their API credentials
must never be supplied to the workload account or inherited by Core's workflow
children. A credential-name denylist is not a substitute for that separation.

The host must protect policy/trust inputs, Core and plugin binaries, evidence,
and final reports from the tested process. Content snapshots and policy pins
detect supported changes; they do not prevent a process with the same operating
system privileges from tampering and restoring bytes. Native build-cache setup
also remains a host responsibility. No cache manager, volume, installer or
recovery service is introduced here.

Results record the observed runtime digest and explicitly leave environment
binding and isolation attestation unproven. They do not certify that these host
conditions were met. Missing evidence, invalid signatures, failed checks and
reported cleanup errors retain their normal failure semantics. An external
consumer must reject a result for a use requiring isolation that has not been
independently established. This increment does not close all of issue #132's
production execution-trust acceptance conditions.
