# Verification Plan Specification

## Implementation Plan and Timeline

This specification is the contract implemented by this change. The following
sequence records the delivery order and evidence gates for the implementation
and its follow-up acceptance work.

| Window | Deliverable | Evidence gate |
| --- | --- | --- |
| T+0 to T+1 day | Capture the fixed-orchestration reference and approve the private model and closed gate registry | Inventory and field registry reviewed; no behavior changed |
| T+2 to T+3 days | Specify plan construction, legacy DAG synthesis, explicit-gate validation, and diagnostics | Graph fixtures cover closure, cycles, references, profiles, and scopes |
| T+4 to T+5 days | Specify adapter result mapping, failure propagation, cancellation, and cleanup | Boundary fixtures cover success, failure, timeout, cancellation, and cleanup |
| T+6 to T+7 days | Run compatibility review and accept the separately scoped implementation | CLI/report/error snapshots match the fixed reference; ADR-0027 and this change are implemented |

The implementation order SHALL follow the dependencies above. A scheduler,
new gate, report-schema migration, or public API requires a separate reviewed
change.

## Technical Examples

The configuration discriminator described by this specification is limited to
the following reviewed vocabulary:

```toml
[[steps]]
kind = "builtin-gate"
id = "builtin.secret-scan"
label = "secret scan"
gate_type = "secret-scan"
profiles = ["full"]
```

An absent `kind` retains the legacy external-step interpretation. The private
result contract is logically equivalent to:

```text
NodeResult {
  id, kind, status, duration,
  safe_detail, optional_artifact_path,
  typed_failure_or_cancellation
}
```

These examples describe data contracts only. They do not prescribe Rust types,
module names, or executable behavior.

## ADDED Requirements

### Requirement: Unified internal verification plan

The system SHALL construct a private `VerificationPlan` before any service
startup, subprocess creation, report write, or scheduler operation. Every
selected built-in gate and configured external step SHALL be represented by a
`PlanNode` with a unique deterministic ID, node kind, label, dependencies, and
typed execution specification.

The plan SHALL distinguish built-in gate nodes from external-step nodes and
SHALL reuse the validated `depends_on` closure and stable topological ordering
defined by ADR-0024.

#### Scenario: Legacy selection creates one plan

- **WHEN** a valid legacy v2 configuration is selected for verification
- **THEN** the plan contains secret scan, architecture audit, and the selected
  external dependency closure as nodes
- **AND THEN** every node has a deterministic ID and kind before execution
- **AND THEN** no service, subprocess, or report side effect occurs during plan
  construction

#### Scenario: Invalid plan graph fails closed

- **WHEN** plan construction encounters a duplicate node ID, missing dependency,
  or cycle
- **THEN** verification fails before any node executes
- **AND THEN** the error identifies the invalid plan relation without silently
  dropping or rewriting a node

### Requirement: Closed built-in-gate declaration

The configuration SHALL support an explicit `kind = "builtin-gate"` declaration
with a versioned, closed `gate_type` vocabulary. This change SHALL recognize
only `secret-scan` and `architecture-audit`. An absent `kind` SHALL preserve the
existing external-step interpretation.

Unknown gate types, invalid kind/field combinations, duplicate reserved IDs,
and undeclared node references SHALL be rejected before execution. A built-in
gate SHALL NOT fall back to an external executable.

#### Scenario: Valid built-in declaration is typed

- **WHEN** a configuration declares `kind = "builtin-gate"` with
  `gate_type = "secret-scan"` or `"architecture-audit"`
- **THEN** the plan builder creates the corresponding built-in node
- **AND THEN** its adapter is the existing typed secret or audit operation
- **AND THEN** no command string is inferred from the declaration

#### Scenario: Unknown gate type fails closed

- **WHEN** a configuration declares an unsupported `gate_type`
- **THEN** configuration or plan validation fails before execution
- **AND THEN** the system does not treat it as an external step or ignore it

### Requirement: Legacy default DAG equivalence

When built-in nodes are absent, the system SHALL synthesize internal nodes and
edges equivalent to `builtin.secret-scan -> builtin.architecture-audit ->` each
selected external step. Existing external `depends_on` edges and transitive
closure SHALL remain unchanged. Synthesized nodes and edges SHALL NOT be written
to `flow.toml`.

The default DAG SHALL produce the same observable order as the legacy fixed
orchestration for valid configurations, including `step <id>` selection.

#### Scenario: Fixed gate order is represented as a DAG

- **WHEN** a legacy configuration selects multiple external steps with no gate
  declarations
- **THEN** the topological order begins with secret scan, then architecture
  audit, then the stable external dependency order
- **AND THEN** the serialized configuration remains byte-for-byte unchanged

#### Scenario: External closure remains stable

- **WHEN** selected step `c` depends transitively on `b` and `a`
- **THEN** the plan includes `a`, `b`, and `c` after the mandatory gate chain
- **AND THEN** their relative order matches ADR-0024

#### Scenario: Explicit gates cannot bypass safety ordering

- **WHEN** a configuration explicitly declares built-in nodes
- **THEN** validation verifies their IDs, gate types, and mandatory ordering
- **AND THEN** a declaration that bypasses a required gate is rejected unless a
  separate reviewed policy explicitly permits it

### Requirement: Common node result model

Built-in gates and external steps SHALL map adapter outcomes into one internal
`NodeResult` model with node ID, kind, status, duration, safe detail, optional
artifact path, and typed failure/cancellation reason. Status SHALL distinguish
`passed`, `failed`, `cancelled`, and `skipped`.

The existing public verification report and CLI output SHALL remain compatible;
internal node fields SHALL NOT be serialized publicly without a separate
reviewed contract.

#### Scenario: Built-in and external success share lifecycle

- **WHEN** secret scan, architecture audit, and an external step all pass
- **THEN** each produces a passed `NodeResult`
- **AND THEN** results are emitted in plan order
- **AND THEN** existing labels, report paths, and summary fields remain valid

#### Scenario: Skipped node is not a pass

- **WHEN** an ancestor failure prevents a dependent node from starting
- **THEN** the dependent node is represented as skipped or blocked according to
  the documented policy
- **AND THEN** it cannot satisfy any downstream dependency or overall pass
  condition

### Requirement: Unified failure propagation

The executor SHALL apply one prerequisite-aware failure policy to built-in and
external nodes. Secret-scan failure SHALL prevent architecture audit and
external nodes. Architecture-audit failure SHALL prevent external nodes. An
external failure SHALL block its descendants while preserving the existing
continuation behavior for unrelated selected external nodes.

#### Scenario: Secret gate failure stops downstream work

- **WHEN** secret scan reports findings or a secret adapter error
- **THEN** architecture audit and external nodes do not execute
- **AND THEN** the verification report records the secret gate result using the
  established label and artifact path
- **AND THEN** no later node is reported as passed

#### Scenario: Audit failure stops external work

- **WHEN** secret scan passes and architecture audit fails
- **THEN** no external step starts
- **AND THEN** the audit result remains reportable with its existing error and
  report contract

#### Scenario: External failure blocks descendants only

- **WHEN** external node `a` fails and node `b` depends on `a`
- **THEN** `b` is not executed and is not passed
- **AND THEN** an unrelated selected node follows the pre-existing continuation
  policy

### Requirement: Stable error-code and cancellation behavior

The unified plan executor SHALL preserve `E1402` for cancellation, `E1403` for
execution failures, and `E1404` for report-write failures. Cancellation SHALL
be observed before each node and during running external work. Cleanup SHALL
remain effective on success, failure, timeout, and cancellation.

#### Scenario: Cancellation interrupts an external node

- **WHEN** cancellation is received while an external node is running
- **THEN** the existing process-tree termination boundary is used
- **AND THEN** the running node is recorded as cancelled
- **AND THEN** unstarted descendants do not begin
- **AND THEN** the command returns `E1402` and preserves existing cancellation
  output/report behavior

#### Scenario: Cancellation before the next node

- **WHEN** cancellation is observed after one node completes but before the next
  node starts
- **THEN** no new node starts
- **AND THEN** pending nodes are not reported as passed

### Requirement: Legacy CLI and report compatibility

For a valid legacy configuration, the default-DAG implementation SHALL preserve
CLI stdout/stderr ordering, ANSI policy, report paths, report names, report
fields, labels, summary semantics, and error-code forms. Compatibility SHALL be
verified against the current fixed orchestration using the same fixtures.

#### Scenario: Legacy success is observationally equivalent

- **WHEN** the same valid fixture is run through the fixed-orchestration
  reference and synthesized-DAG implementation
- **THEN** the node order and pass summary are identical
- **AND THEN** `test_result.json`, `test_result.md`, and per-step log paths retain
  their established shape and names

#### Scenario: Legacy failure preserves public codes

- **WHEN** a fixture exercises gate failure, external failure, cancellation, or
  report-write failure
- **THEN** gate and external execution failures retain `E1403`
- **AND THEN** cancellation retains `E1402`
- **AND THEN** report-write failure retains `E1404`
- **AND THEN** no internal node ID or implementation-only status leaks into
  existing output without an approved contract update

## Alternatives Considered

### Keep built-in gates as a hard-coded preamble

Rejected because gate dependencies, result states, cancellation, and report
adaptation would remain split across separate orchestration paths.

### Treat built-in gates as external commands

Rejected because typed gate errors, cleanup ownership, report artifacts, and
secret-handling guarantees cannot be safely represented by arbitrary commands.

### Require users to declare every gate dependency

Rejected because it would make existing configurations unsafe or require
unreviewed file rewrites. Internal synthesis preserves the established safety
chain without mutating user configuration.

### Add parallel scheduling in this change

Rejected because scheduling, resource locking, and cancellation fan-out require
independent safety and performance decisions.

## Rollback Plan

If implementation evidence fails compatibility review, revert the separately
scoped plan-builder and adapter change as one reviewed change. Existing v2
configuration files and reports must remain readable, and rollback must not
rewrite `flow.toml`, remove configured dependencies, or delete published
reports. A rollback returns execution to the fixed orchestration while leaving
this specification and ADR marked proposed for follow-up.

### Requirement: Scope-limited implementation

This change SHALL implement only the private verification-plan boundary and
the compatibility adapter described above. It SHALL NOT add a scheduler, a new
business gate, a renderer, service locks, retries, or unrelated production
behavior. Any relaxation of this boundary SHALL be delivered in a separately
reviewed change that references this specification.

#### Scenario: Scope review detects prohibited expansion

- **WHEN** this change is reviewed before acceptance
- **THEN** the change contains only the private plan, typed gate declarations,
  compatibility adapter, focused tests, and related documentation
- **AND THEN** no scheduler, new business gate, renderer, lock, retry, or
  unrelated workflow behavior is present
