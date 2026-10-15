# Inspect a verification plan

Use the existing verify command with --plan. Add --json for the
[verification-plan/v1 schema](../schema/verification-plan.schema.json). There is
one planning interface; scope remains the narrower path/component report.

~~~bash
harness-gate verify --plan
harness-gate verify --plan --staged --profile hook
harness-gate verify --plan --base origin/main --profile full --json
harness-gate verify --plan --all --json
harness-gate verify --plan --components backend,frontend --profile full --json
~~~

Explicit components conflict with staged/base/all, as in execution. A profile
comes from --profile or project.default_profile. The check alias also accepts
--plan. Bare verify --json is rejected to avoid suggesting a JSON execution mode.

A successful preview exits 0 with status planned. This means the selection plan
was constructed, never that verification passed. verification_result is always
null; runtime_inputs is not_validated. Invalid configuration, unknown components
or profiles, missing required configuration, unmatched=fail and dependency errors
exit 1 with status invalid, plan=null and PLAN_INVALID diagnostics containing
a concrete explanation plus structured configuration diagnostics. No partial
plan is presented as a verification result. CLI usage errors retain Clap's
normal nonzero usage output.

The plan lists:

- Paths and every matching rule index; both rename endpoints participate.
- All rules, flow components and profiles, with inclusion/exclusion reasons.
- Selected nodes in execution's stable topological order, then excluded steps
  in configuration order. execution_order contains exactly the selected nodes.
- Configured dependencies and the mandatory built-in chain. Secret scanning and
  architecture audit remain required even when scope selects no external step.
  Dependency closure can include steps outside the chosen component/profile.
- Every configured service, its selected consumers and required check bindings.
  Service connection strings and environment values are not printed or probed.
- Quality profile participation, selected/omitted collectors and policies,
  capability/series bindings, component/subject declarations, workflow state,
  trusted-key and baseline-request paths, and baseline requirements.

The renderer and JSON use the same model and reason messages. Arrays have
deterministic ordering for unchanged configuration/Git inputs; there are no
invocation IDs, timestamps or private snapshot paths in a valid plan. Rules use
zero-based configuration indices. Reasons are versioned codes:

| Area | Codes |
| --- | --- |
| Paths | PATH_RULE_MATCH, UNMATCHED_ALL, UNMATCHED_IGNORED |
| Rules | RULE_MATCHED, RULE_NOT_MATCHED |
| Components | EXPLICIT_COMPONENT, ALL_COMPONENTS, UNMATCHED_FALLBACK_ALL, SCOPE_RULE_COMPONENT, COMPONENT_IMPACT, COMPONENT_NOT_AFFECTED |
| Profiles | PROFILE_REQUESTED, PROFILE_DEFAULT, PROFILE_NOT_SELECTED |
| Steps | MANDATORY_BUILTIN_GATE, COMPONENT_AND_PROFILE, DEPENDENCY_REQUIRED, COMPONENT_NOT_SELECTED, PROFILE_NOT_INCLUDED |
| Services | SERVICE_REQUIRED, SERVICE_NOT_REQUIRED |
| Quality | QUALITY_NOT_CONFIGURED, COLLECTOR_SELECTED, COLLECTOR_NOT_SELECTED, POLICY_SELECTED, POLICY_NOT_SELECTED |

Scope detection, compiled rule matchers, explicit-component validation,
VerificationPlan::build and dependency closure are shared with execution.
No planner-specific scope fallback or profile expansion is introduced.
An unmatched=all fallback requires an actual unmatched path; a clean tree still
plans only the mandatory built-ins. Rename detection preserves both old/new paths
regardless of Git rename settings, including names containing spaces/newlines.

## Read-only boundary

Preview bypasses Project::prepare, report allocation, the scheduler, service
management, quality preparation/collection, baseline resolution and signed
adapter authentication. It never starts project commands, probes tools/services,
fetches a baseline, authenticates or consumes a signed request, updates a replay
ledger, or creates report/lease directories. It reads and validates configuration
and policy documents and performs necessary Git metadata reads. Git optional
index writes, filesystem-monitor hooks and external diff/text conversion commands
are disabled for scope reads.

For staged mode, a private temporary configuration view preserves indexed path
existence and reads only flow/quality/policy blobs from the index. It never calls
write-tree or checkout-index, runs checkout filters, or reads source/request
bodies. The temporary directory is removed when inspection finishes. Conflicted,
symlink or submodule index entries are explicitly rejected; this is a diagnostic,
not a claim that an incomplete staged view is valid. Configuration with a checkout
filter, working-tree-encoding or ident transformation in either indexed or
working-tree attributes is rejected: preview cannot
run that filter or promise the configuration execution would obtain. Ordinary
source-file filters do not run and do not prevent a plan.

Runtime executable availability, environment values, signed-request authenticity,
nonce freshness, evidence digests and service readiness remain unvalidated.
Quality workflow state and request paths are requirements, not approvals.
A later Git/configuration change can produce a different plan; run verify normally
to obtain the authoritative result. No product protocol, collector measurement-series, requiredness,
threshold, baseline/debt/ratchet, waiver or publication authority is changed.

## Compatibility and rollback

This is an additive CLI/output interface. Existing flow v2, quality v1, scope JSON
and verification reports are unchanged. Consumers should check the schema marker;
a future incompatible plan change requires a new version. Revert the plan flag,
schema and explanation modules together to remove the interface. Historical
evidence and baselines must not be reset during rollback.

Git-backed previews require Git with --no-lazy-fetch support. Missing partial-clone objects fail with diagnostics instead of launching remote helpers, fetching data or writing Git objects. Existing execution commands retain their normal Git behavior. Preview also omits the optional HEAD object probe because it does not produce an execution source identity.

Core self-measurement selection expands to gh29-verify-plan/1, with fresh base/head collection under the same expanded inventory; see the [measurement migration](quality/issue-29/README.md). Historical reports, thresholds and debt/ratchet rules are preserved.

Component impact version 1 adds `impact_chain` (a deterministic shortest witness),
`impact_sources` (original changed paths and rule indices), and
`depends_on_components` (the full declared adjacency) to component explanations.
See [component impact configuration](configuration.md#explicit-component-impact-optional).
