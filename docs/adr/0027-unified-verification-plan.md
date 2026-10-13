# ADR-0027: Unify Built-in Gates and Configured Steps in a Verification Plan

## Status

**Accepted** (2026-08-29)

## Context

ADR-0024 introduced `depends_on`, dependency closure, and deterministic
topological ordering for configured verification steps. The execution boundary
still treats the secret scan and architecture audit as special operations that
run before configured steps. This split creates several architectural gaps:

- there is no explicit internal `VerificationPlan` or `PlanNode` model;
- built-in gates and external steps do not share one dependency and result
  contract;
- the configuration cannot declare a built-in gate using a stable `kind` and
  `gate_type` vocabulary;
- legacy configurations have no represented graph, so the equivalence between
  the fixed `secret -> audit -> external steps` sequence and a default DAG is
  implicit rather than testable; and
- error propagation, cancellation, skipped descendants, and report ordering
  are implemented at different layers.

The next execution phase needs one plan abstraction before parallel scheduling,
additional gates, or richer cancellation policies are considered. It must also
preserve current behavior for existing v2 configurations: secret scanning runs
first, architecture auditing runs only after a successful secret scan, and
configured external steps run only after both gates pass.

## Decision

Introduce a private, execution-oriented verification plan that represents every
gate and configured step as a node in one directed acyclic graph. This decision
defines the model and compatibility contract only; it does not authorize a
parallel scheduler, a new business gate, or changes to existing report files.

### 1. VerificationPlan and PlanNode model

The internal model has these logical concepts:

| Concept | Required fields and semantics |
| --- | --- |
| `VerificationPlan` | Ordered `nodes`, dependency edges, selected scope/profile, and an execution policy. It is built after configuration validation and selection, before any service, subprocess, or report side effect. |
| `PlanNode` | Stable `id`, display `label`, `kind`, dependency IDs, selection state, and an execution specification. IDs are unique within a plan and deterministic across equivalent inputs. |
| `PlanNodeKind::BuiltinGate` | A built-in Harness-Gate operation identified by a closed `gate_type` enum. Initial values are `secret-scan` and `architecture-audit`. |
| `PlanNodeKind::ExternalStep` | A reference to one configured `[[steps]]` entry. Its node ID remains the configured step ID for compatibility. |
| `NodeResult` | Common internal result containing node ID, label, kind, status, duration, detail, report/log path where applicable, and a typed failure/cancellation reason. |

The plan is an internal model. It must not expose a new public Rust API or
serialize implementation-only fields into existing reports unless a separate
compatibility decision approves that change. A node may be selected, skipped
because an ancestor failed, cancelled, or executed successfully/unsuccessfully.
The model must distinguish `failed`, `cancelled`, and `skipped` so downstream
behavior is not inferred from a Boolean alone.

The plan builder must reject duplicate node IDs, missing dependencies, cycles,
and references to unknown configured steps before execution. It reuses
ADR-0024's transitive dependency closure and stable topological tie-breaking;
the plan must not introduce a second ordering algorithm.

### 2. Built-in gate declaration

Extend the configuration vocabulary with an explicit built-in-gate declaration
for reviewed gate nodes:

```toml
[[steps]]
kind = "builtin-gate"
id = "builtin.secret-scan"
label = "secret scan"
gate_type = "secret-scan"
profiles = ["full"]
```

The exact set of accepted `gate_type` values is versioned and closed. The
initial implementation must support only the existing built-in operations:
`secret-scan` and `architecture-audit`. A future gate type requires its own
decision covering configuration fields, side effects, report output, error
code, and security review. An unknown `gate_type` is a configuration error; it
must never fall back to an external executable or be silently ignored.

For backward compatibility, the existing external-step shape remains valid
when `kind` is absent and continues to mean `external-step`. New declarations
must not overload fields whose meaning differs between node kinds. A schema
version bump is required only if the discriminator cannot be added compatibly
to the current v2 model.

### 3. Default DAG for legacy configurations

When a legacy v2 configuration does not declare built-in gate nodes, the plan
builder synthesizes deterministic internal nodes:

```text
builtin.secret-scan
        |
builtin.architecture-audit
        |
selected external steps and their configured dependency closure
```

Every selected external step has an implicit dependency on
`builtin.architecture-audit`. The architecture-audit node depends on
`builtin.secret-scan`. Existing `depends_on` edges between external steps are
retained unchanged. The synthesized edges are internal plan edges and are not
written back to `flow.toml`.

The resulting topological order is therefore equivalent to the current fixed
sequence for every valid legacy configuration. If a future configuration
explicitly declares the built-in nodes, the builder verifies that their
`gate_type` and required ordering match this compatibility chain; it does not
allow an explicit declaration to bypass a mandatory safety gate silently.

`step <id>` uses the same rule: the selected step's dependency closure is
expanded, and the default built-in gate chain remains before the first external
node. A separate reviewed option is required to run a gate in isolation.

### 4. Unified execution and result semantics

The executor consumes `VerificationPlan` nodes in stable topological order.
Built-in gates and external steps share the same lifecycle:

1. mark the node as running and record a monotonic start time;
2. execute the node through its owned adapter;
3. convert the adapter outcome into `NodeResult`;
4. publish the result to the plan report/output adapter; and
5. make the result available to dependent nodes.

The default policy is fail-fast for mandatory prerequisites:

- a failed secret scan prevents architecture audit and all external steps;
- a failed architecture audit prevents all external steps;
- a failed external step marks that node failed and prevents only its
  descendants; unrelated selected nodes retain the existing serial execution
  policy and may continue when the current behavior permits it; and
- a skipped node is never reported as passed.

The exact continuation behavior for unrelated external steps must match the
current `run_configured_steps` contract and be captured before refactoring.
Changing it requires a separate ADR rather than being implied by the plan
model.

`NodeResult` is the source of truth for internal propagation. The existing
`TaskResult`-shaped `test_result.json` and Markdown output remain compatible:
built-in results retain the established labels (`secret scan` and
`architecture audit`), report paths, duration fields, and pass/fail summary.
Any new status or node-kind fields in public reports are deferred to a reviewed
report-schema change. Error mapping remains stable: cancellation continues to
use `E1402`, execution failures use `E1403`, and report-write failures use
`E1404`; built-in adapters map their existing typed errors through the same
verification boundary.

### 5. Cancellation and cleanup contract

Cancellation is a plan-level event with node-level observation points. On a
received cancellation signal:

- no not-yet-started node may begin;
- the currently running external task is terminated through the existing
  process cancellation boundary;
- the current node is recorded as `cancelled`, descendants are recorded as
  `skipped` or `cancelled` according to the existing report policy, and no
  node is reported as passed; and
- service/process cleanup remains owned by the existing adapters and must run
  on success, failure, timeout, and cancellation.

The plan abstraction must not swallow cancellation as an ordinary gate
failure, retry it implicitly, or change signal/error-code behavior. Parallel
execution and cancellation fan-out are explicitly deferred; this ADR defines
the invariant that any later scheduler must preserve.

## Implementation Evidence

The private plan, built-in gate registry, synthesized legacy DAG, unified
result mapping, failure propagation, cancellation handling, and compatibility
adapter were implemented in [PR #35](https://github.com/musutrade/Harness-Gate/pull/35).
The cross-platform and quality evidence passed in
[main CI run 33167763113](https://github.com/musutrade/Harness-Gate/actions/runs/33167763113),
including the CLI contract and report snapshots. Parallel scheduling remains a
separate follow-up under ADR-0028.

### 6. Compatibility evidence

Before accepting this decision's implementation, add regression coverage for:

1. a legacy configuration producing the synthesized
   `secret -> audit -> external` DAG;
2. explicit external `depends_on` closure and stable topological order;
3. an explicit built-in-gate declaration with valid and unknown `gate_type`;
4. secret-gate failure preventing audit and external nodes;
5. audit-gate failure preventing external nodes;
6. external failure propagation to descendants while preserving the documented
   behavior of unrelated nodes;
7. cancellation, timeout, and cleanup through the unified result path; and
8. byte-for-byte or field-level compatibility of CLI output, report paths,
   report shape, and `E1402`/`E1403`/`E1404` error codes for the legacy fixed
   sequence.

The compatibility suite must compare the old fixed orchestration against the
default-DAG plan for the same fixtures. A passing graph test alone is
insufficient: observable output, ordering, failure propagation, and cleanup
must also be asserted.

## Consequences

### Positive

- Built-in gates and external steps acquire one explicit dependency and result
  boundary, making later scheduling and additional gates reviewable.
- Legacy behavior is represented as data and can be checked rather than being
  hidden in control-flow branches.
- Cancellation, failure, skipped descendants, and report ordering have one
  vocabulary and one compatibility surface.
- Configuration authors can eventually declare reviewed built-in gates without
  inventing a separate execution mechanism.

### Negative

- The plan builder and unified result model add indirection to a currently
  straightforward serial flow.
- The configuration discriminator and closed `gate_type` registry become a
  schema and migration compatibility surface.
- Preserving legacy report fields while adding internal statuses requires an
  explicit adapter and additional regression fixtures.
- A future scheduler must preserve the synthesized gate chain and resource
  constraints from ADR-0026, limiting otherwise attractive scheduling
  shortcuts.

## Alternatives Considered

### Keep gates as special preamble code

Rejected. It leaves dependency, result, and cancellation semantics split and
makes each new gate another orchestration exception.

### Convert gates into shell commands

Rejected. Built-in gates have typed configuration, reports, cleanup, and error
semantics that cannot safely be represented as arbitrary external commands.
It would also weaken error-code and secret-handling guarantees.

### Replace the legacy sequence with user-authored dependencies immediately

Rejected. Existing configurations would need silent or manual edits, and a
missing edge could bypass a mandatory gate. The compatibility DAG must first be
synthesized internally.

### Expose `VerificationPlan` as a public library API

Rejected. Harness-Gate is currently a binary-oriented tool; an internal model
keeps the plan free to evolve while preserving the CLI and report contracts.

### Add parallel execution in the same change

Rejected. Parallel scheduling, service locking, and cancellation fan-out have
independent safety and performance decisions. ADR-0026 defines the static
resource boundary they must consume later.

## References

- [ADR-0010: Decompose the Verification Module by Responsibility](0010-verify-module-decomposition.md)
- [ADR-0024: Add Dependency Ordering to Verification Steps](0024-verification-plan-dependencies.md)
- [ADR-0025: Establish Phase 1 Quality Baseline Gates](0025-phase-1-quality-baseline-gates.md)
- [ADR-0026: Add Configuration Safety Diagnostics and Future-Concurrency Preflight](0026-configuration-safety-diagnostics.md)
