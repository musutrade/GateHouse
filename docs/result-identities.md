# Policy, runtime and historical results

Machine results retain their existing candidate, configuration, invocation,
quality evidence and compilation bindings. The additive `identities` object
separates quality policy from recorded execution facts; it does not replace
those existing fields or introduce a second quality evaluator.

`quality_policy_identity` binds workflow policy, selected profile, requiredness,
waivers, built-in policy files and the quality compiler's selected original
rules, baseline policy, mappings and exceptions. The compiler retains its full
`identity` for concrete candidate/subject/series/run binding. Hashing policy
rules before expansion avoids treating a new source fingerprint or run ID as
a policy edit. CLI/help source, report presentation, external delivery settings,
operator probes and cache configuration are not policy inputs.

`runtime_bundle_digest` binds the executing Core file digest, platform,
execution and runner configuration, and the effective collector host policy.
These facts are captured before workload execution. Collected quality inputs
add their existing tool/runtime series; authenticated producer facts remain
available alongside them. Raw command arguments, service configuration and
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
