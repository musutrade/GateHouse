# Design: Unified Verification Plan and Gate Execution Contract

## Scope Boundary

This design is normative for the Phase 3 execution boundary implemented by this
change. It defines the data, graph, compatibility, and evidence contracts. The
implementation remains limited to this plan boundary; it does not add a
scheduler or change business gates, and it does not authorize a public library
API.

## Existing Behavior to Preserve

For a valid legacy configuration, verification currently performs:

```text
secret scan -> architecture audit -> selected external steps
```

The audit runs only after a passing secret scan. External steps run only after
both mandatory gates pass. Selected external steps include their transitive
`depends_on` closure and use ADR-0024's stable topological order. Existing
report labels, report paths, output streams, and error codes are compatibility
surfaces.

## 1. Internal Model

The implementation plan SHALL define private types equivalent to the following
logical model. Names may differ only with an ADR-approved rationale.

```text
VerificationPlan {
  nodes: ordered collection<PlanNode>
  selected_profile: profile identifier
  selected_scope: scope result
  policy: ExecutionPolicy
}

PlanNode {
  id: stable node identifier
  label: display label
  kind: BuiltinGate(gate_type) | ExternalStep(step_id)
  depends_on: ordered node identifiers
  selection: selected | prerequisite
  execution: typed adapter specification
}

NodeResult {
  id: node identifier
  kind: node kind
  status: passed | failed | cancelled | skipped
  duration: measured duration
  detail: safe optional detail
  report_or_log: optional existing artifact path
  failure: optional typed reason
}
```

The model must distinguish `failed`, `cancelled`, and `skipped`; a Boolean
`passed` value is not sufficient for dependency propagation. Node IDs are
unique within a plan and deterministic for equivalent configuration, scope,
profile, and selection inputs. A plan is fully validated before service setup,
subprocess creation, report writing, or other execution side effects.

`VerificationPlan` and `NodeResult` remain private implementation details.
Existing `TaskResult` and `VerificationReport` serialization is unchanged by
this specification. An adapter may project `NodeResult` into the current
report shape, but must not silently add or rename public fields.

## 2. Node Identity and Kinds

### Built-in nodes

The reserved built-in IDs are:

| ID | `gate_type` | Existing operation | Legacy label |
| --- | --- | --- | --- |
| `builtin.secret-scan` | `secret-scan` | staged or working-tree secret scan | `secret scan` |
| `builtin.architecture-audit` | `architecture-audit` | architecture audit | `architecture audit` |

The IDs and `gate_type` values are versioned identifiers, not display text.
They are not executable program names and cannot be redirected to an external
command. A future gate type requires a separate proposal that defines its
configuration, side effects, report, cleanup, and error mapping.

### External nodes

An external node references exactly one existing configured step. Its node ID
is the configured step ID, preserving current selection and report identity.
The node carries the step's services, parser, command, timeout, log, and
`depends_on` data through a typed execution specification; it does not duplicate
or reinterpret those fields.

## 3. Configuration Discriminator

The configuration contract SHALL support an explicit discriminator equivalent
to:

```toml
[[steps]]
kind = "builtin-gate"
id = "builtin.secret-scan"
label = "secret scan"
gate_type = "secret-scan"
profiles = ["full", "hook"]
```

When `kind` is absent, the existing step shape retains its meaning as an
external step. If the current schema cannot add the discriminator without
ambiguity, a separate schema-version decision is required before implementation.

The accepted `gate_type` set is closed to `secret-scan` and
`architecture-audit` for this change. Unknown values, duplicate reserved IDs,
invalid kind/field combinations, and references to undeclared nodes fail before
execution. A built-in declaration must not accept external-only fields unless
the future gate contract explicitly defines them.

Explicit built-in declarations are opt-in syntax, not an escape hatch. The
builder must verify that mandatory safety ordering is retained and must reject a
declaration that bypasses the secret or architecture gate without an explicit,
separately reviewed policy.

## 4. Legacy Default DAG

When no built-in nodes are declared, the builder synthesizes internal nodes and
edges:

```text
builtin.secret-scan
        |
builtin.architecture-audit
        |
selected external nodes + their external dependency closure
```

For every selected external node, add an internal edge from
`builtin.architecture-audit` to that node. Retain all configured external
`depends_on` edges. Do not write synthesized IDs or edges to `flow.toml` and do
not mutate the selected scope or profile.

The default DAG must be topologically equivalent to the fixed orchestration:

1. secret scan is first;
2. architecture audit follows a passing secret scan;
3. external prerequisites and selected steps follow a passing audit, in the
   same stable order as ADR-0024; and
4. no external node starts if either mandatory gate fails.

For `step <id>`, expand the requested step's external dependency closure and
place the same built-in chain before the first external node. Running a built-in
node alone requires a future, explicitly reviewed command contract.

## 5. Plan Construction Phases

The future implementation should expose one auditable construction sequence:

```text
validated FlowConfig
  -> select profile/scope/requested step
  -> add explicit external nodes
  -> expand external dependency closure
  -> synthesize or validate built-in nodes
  -> add mandatory gate edges
  -> validate node IDs, references, cycles, and resources
  -> stable topological order
  -> execute
```

Configuration diagnostics and ADR-0026 resource preflight run before this plan
executes. Plan construction must not inspect service values, start Docker,
launch a command, write a report, or resolve a template.

The plan builder reuses the existing stable topological ordering algorithm. It
must define deterministic tie-breaking for synthesized nodes and configured
nodes, and must document the choice in tests. It must not create a second,
inconsistent dependency relation.

## 6. Unified Result and Failure Semantics

Every node adapter follows the same lifecycle:

1. transition the node to running;
2. record a monotonic start time;
3. execute through the owned built-in or external adapter;
4. map the adapter outcome to `NodeResult`;
5. publish the result to the existing report/output adapter; and
6. evaluate dependent-node eligibility.

The default policy is serial and prerequisite-aware:

| Event | Required propagation |
| --- | --- |
| Secret scan fails | Architecture audit and every external descendant are not started. The report records the secret gate failure using the existing label/path contract. |
| Architecture audit fails | External nodes are not started. The report records the audit failure using the existing label/path contract. |
| External node fails | The node is failed; its descendants are skipped or otherwise blocked according to the existing documented policy; unrelated selected nodes retain current serial behavior. |
| Node is skipped | It is never counted as passed and cannot satisfy a dependency. |
| Adapter/report error | Preserve the existing typed verification boundary and error-code mapping. |

The implementation must first capture current behavior for unrelated external
steps. This OpenSpec does not authorize changing whether they continue after a
failure. Any changed continuation policy requires a separate ADR.

Error mapping remains:

| Condition | Code |
| --- | --- |
| cancellation | `E1402` |
| execution failure | `E1403` |
| report-write failure | `E1404` |

Built-in adapter errors map through the same verification boundary and must not
be downgraded to generic process failures. Diagnostic details must not reveal
secret values or service connection contents.

## 7. Cancellation and Cleanup

Cancellation is observed before starting each node and while an external task
is running. The contract is:

- no not-yet-started node begins after cancellation is observed;
- the running external process uses the existing process-tree termination path;
- the current node is `cancelled`, not merely `failed`;
- dependent nodes become `skipped` or `cancelled` according to the established
  report policy and never `passed`;
- service and process cleanup runs on success, failure, timeout, and
  cancellation; and
- the command returns `E1402` with the existing user-visible cancellation
  behavior.

This change does not define parallel cancellation fan-out, retries, or a new
timeout policy. A future scheduler must preserve these invariants.

## 8. Public Compatibility Adapter

The plan layer must adapt results to the current public surfaces:

- preserve `secret scan` and `architecture audit` labels;
- preserve `test_result.json`, `test_result.md`, and per-step log locations;
- preserve existing fields and summary semantics;
- preserve stdout/stderr ordering under `--color never` and interactive output
  policy; and
- preserve `E1402`, `E1403`, and `E1404` mappings.

If a plan-specific status, node kind, or dependency field is useful to users,
it must be introduced through a separately reviewed report-schema/CLI contract
change. The internal plan must not leak implementation-only node IDs into
legacy reports without that review.

## 9. Acceptance and Rollback

Acceptance requires the regression matrix in `specs/verification-plan/spec.md`
and the task evidence in `tasks.md`. The matrix must compare legacy fixed
orchestration with the synthesized DAG on the same fixtures, not merely assert
that a graph can be constructed.

Rollback is a single reviewed revert of the plan builder, node adapters, and
configuration discriminator. Existing configurations and reports must remain
readable. No rollback may silently rewrite a user's `flow.toml`, remove a
declared dependency, or change an already-published report.

## Alternatives Considered

### Keep gates as special preamble code

Rejected because dependency, result, cancellation, and reporting semantics stay
split and every future gate adds another orchestration exception.

### Model gates as arbitrary external commands

Rejected because secret/audit operations have typed errors, reports, cleanup,
and security boundaries that shell commands cannot safely express.

### Require users to add gate dependencies manually

Rejected because existing configurations would need edits and a missing edge
could bypass mandatory safety gates. Synthesis preserves compatibility first.

### Expose the plan as a public library API

Rejected because the repository currently provides a binary-oriented CLI; a
private model allows evolution without freezing internal fields.

### Implement parallelism together with the plan

Rejected because scheduling, resource locks, and cancellation fan-out require
independent safety and performance decisions.
