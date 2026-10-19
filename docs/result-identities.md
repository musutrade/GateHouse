# Policy, runtime and historical results

Machine results retain their existing candidate, configuration, invocation,
quality evidence and compilation bindings. The additive `identities` object
separates quality policy from recorded execution facts; it does not replace
those existing fields or introduce a second quality evaluator.

`identities.quality_policy_identity` is the exact `quality-policy/v1` value
from compiled quality inputs, or null when no compiled identity is available.
It binds the selected original rules, workflow policy, requiredness, waivers,
built-in policy files, baseline policy, mappings and exceptions. The compiler
retains its full `identity` for concrete candidate/subject/series/run binding.
Hashing rules before expansion avoids treating a new source fingerprint or run
ID as a policy edit. CLI/help source, report presentation, external delivery
settings, operator probes and cache configuration are not policy inputs.

`verification_policy_identity` has a distinct `verification-policy/v1` domain.
It binds `{workflow, quality_policy_identity}`, where workflow contains the
projected flow, built-in policy pins, selected `profile` and `only_step` (null
for normal verification). This outer selection binding must not be compared
with the compiler's policy identity as though the two were the same field.

`policy_binding_complete` means a compiled quality policy identity is present.
It is false when compilation is absent or fails, including execution-only
`step` runs. It does not mean quality PASS, complete coverage, or permission to
reuse evidence. Generic workflow identity remains available independently.

`runtime_bundle_digest` binds the executing Core file digest, platform,
execution and runner configuration, and the effective collector host policy.
These facts are captured before workload execution. Collected quality inputs
add their existing tool/runtime series, selected adapter executable/settings
bindings and collected/retained origin labels. The digest covers all three
inside `runtime.facts`: `series`, `collectors`, and `collector_producers`.
Origin labels are not signatures. Collector host limits, including
`max_artifact_bytes`, belong to runtime, not quality thresholds. Raw command arguments, service configuration and
environment values are not copied into runtime metadata. Configurations are
bound through domain-separated digests; ambient secret values are not read.

Runner isolation and execution controls belong to runtime. Required checks,
thresholds, debt, ratchets and waivers belong to policy. A policy-only change can
therefore preserve runtime identity; an execution-only change can preserve
policy identity. A changed Core binary changes runtime identity even if its
version string is unchanged. Neither digest is a permission to reuse a result.

## Applicability is not inferred

The current local runner records `environment_binding = not_attested` and
`isolation_attestation = not_provided`. Its machine result explicitly records
`current_applicability = not_proven`. An ordinary local process cannot certify
its own host isolation, unrecorded transitive tools or ambient environment.
Consumers must reject reuse of an old success when those conditions have not
been established for the current request, even if the policy digest matches.

`report-verify` still verifies historical integrity and reports current
applicability as `not_evaluated`. Its successful exit is not a new quality PASS
or a current execution authorization. Current candidate, baseline, policy,
Core, plugin/toolchain, runtime and evidence must all be checked by a consumer
using independently trusted expectations. Historical results stay unchanged
when a current expectation differs. No automatic rerun or remote Check
revocation occurs.

Older reports without these additive fields remain historically readable.
Missing identity facts cannot be treated as wildcard matches for current use.
This increment records and separates facts; it does not claim complete host
attestation or close the remaining execution-trust acceptance work.

These are additive fields under the existing result schema version. Updated
Core readers retain historical compatibility, but consumers pinned to an older
strict schema must upgrade that schema before accepting new reports. An old
schema rejection must remain an error; do not disable additional-property checks.
