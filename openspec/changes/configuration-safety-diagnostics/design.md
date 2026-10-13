# Design: Configuration Safety Diagnostics and Future-Concurrency Preflight

## Scope Boundary

This design is the normative contract for the implementation in this change.
It adds validation and diagnostics only: serial execution order, report files,
and existing CLI success contracts remain in force. It does not add a scheduler,
an HTML/Tera renderer, or template loading.

## System Model

The final validation boundary will have a single logical flow:

```text
source TOML
  -> interpolation
  -> TOML decode + source map
  -> dedicated environment overrides
  -> field and cross-reference rules
  -> dependency reachability + resource preflight
  -> ordered diagnostic set
  -> human or JSON config-check rendering
```

Any error diagnostic stops the flow before project discovery, service startup,
subprocess creation, report writing, or scheduling. The analysis is static: it
does not read injected service values, inspect Docker, or execute commands.

## 1. Diagnostic Data and Rendering Contract

### Canonical diagnostic model

The implementation will use a typed diagnostic model rather than create errors
by concatenating strings. A diagnostic has the following logical fields:

| Field | Type/constraint | Contract |
| --- | --- | --- |
| `id` | uppercase stable rule identifier | Matches `HGCFG-[A-Z0-9-]+`; identifies a validation rule, not wording. |
| `severity` | `error` or `warning` | All load/validation blockers are `error`; a warning is permitted only for a documented migration advisory. |
| `path` | canonical field path | Required for semantic diagnostics; parser/interpolation diagnostics use the nearest known path or `$`. |
| `message` | safe concise text | States what is invalid; contains no resolved environment value, connection value, or template content. |
| `help` | safe deterministic text | States the repair; never proposes an implicit mutation. |
| `location` | optional source position/range | One-based start line/column, optional end; absent only when no source is available. |
| `related` | ordered list | Each entry has a path, relation label, and optional location; used for the other side of a conflict. |

The canonical path grammar is intentionally independent of TOML table spelling:

```text
$
project.name
steps[2].services[0]
services["test-db"].inject_env
paths.aliases["backend.api"].path
```

Array indices are zero based. Dot notation is used only for identifiers that
match the configured identifier grammar; all map keys that do not qualify are
quoted and JSON-escaped inside brackets. The path string is a compatibility
surface and must be snapshot tested.

Diagnostic IDs are allocated in a documented registry. At minimum, the final
implementation must reserve distinct IDs for TOML parse, malformed/missing
interpolation, invalid field value, unknown reference, dependency error,
service injection collision, shared service resource, duplicate log, and
template path containment. A rule's ID may not change merely because its human
wording changes.

### Aggregation and order

The loader reports every safely collectable independent defect in one run, up
to 50 diagnostics. It may stop a phase when continuing would cause misleading
claims—for example, a syntax error can prevent semantic analysis—but it must
not suppress separate semantic errors solely because another semantic error
exists. The envelope sets `truncated: true` when the 50-diagnostic cap is
reached and emits a final deterministic `HGCFG-DIAGNOSTICS-TRUNCATED` error
with remediation.

Diagnostics with known source locations sort by start line, start column, path,
then ID. Diagnostics without source locations sort after them by path then ID.
Related entries sort by path then relation. The same source and environment
must yield byte-for-byte-equivalent JSON after standard JSON serialization.

### Human and JSON interfaces

The default `harness-gate config check` rendering remains human readable. Each
error includes the configuration file path, diagnostic ID, primary canonical
path, reason, source position when available, any related locations, and a
`help:` line. It preserves the established top-level command failure category
and exit convention. It must not change successful default output unless an
intentional CLI snapshot update is approved.

The later implementation adds an explicit non-default interface:

```text
harness-gate config check --format json
```

Its stdout is exactly one JSON object and its stderr contains no human
diagnostic prose. Its logical envelope is:

```json
{
  "schema_version": 1,
  "valid": false,
  "truncated": false,
  "diagnostics": [
    {
      "id": "HGCFG-SERVICE-INJECT-COLLISION",
      "severity": "error",
      "path": "steps[1].services[0]",
      "message": "service injects TEST_DATABASE_URL already injected by an unordered step",
      "help": "add depends_on or use distinct inject_env names",
      "location": { "line": 42, "column": 12 },
      "related": [
        {
          "path": "steps[0].services[0]",
          "relation": "conflicts-with",
          "location": { "line": 31, "column": 12 }
        }
      ]
    }
  ]
}
```

For a valid configuration, `valid` is `true`, `truncated` is `false`, and
`diagnostics` is an empty array. `schema_version` changes only through an
intentional compatibility record; unknown optional fields may be added, but
existing fields and their meanings remain stable within version 1.

`--format json` returns the same existing nonzero process status as a failed
human config check when `valid` is false. A valid JSON result returns zero.
Neither form serializes values obtained from interpolation, environment
overrides, or service connection resolution.

### Source context

Interpolation is evaluated against source spans so malformed expressions and
missing variables can identify the `${...}` token. TOML decode errors retain
the parser's most precise known position. A source map binds decoded field
paths to TOML key/value spans; semantic diagnostics use those spans for the
primary and related fields. In-memory APIs may return a diagnostic with no
location, but never omit its path, ID, message, help, or related paths.

## 2. Potential-Concurrency and Resource Analysis

### Reachability relation

Let `A -> B` mean that B directly or transitively declares `depends_on = [...,
A, ...]`. Two distinct steps are **potentially concurrent** if neither `A -> B`
nor `B -> A` is true. The graph uses every configured step: profile, scope,
and current serial execution do not prove mutual exclusion because direct
one-step selection, `--all`, and a later scheduler can select different sets.

Dependency validation runs first. Missing, self, and cyclic edges produce their
own diagnostics; resource analysis does not rely on undefined reachability.
Reachability may be calculated through a deterministic DFS/BFS from each node
or an equivalent closure algorithm. Its result must be invariant under hash-map
iteration order.

### Resource declarations

For each step, preflight derives only configuration names:

| Resource | Derivation | Identity |
| --- | --- | --- |
| Injected environment variable | Resolve each `steps[i].services[j]` to `services[id].inject_env` | Uppercase environment-variable name. |
| Service resource | Resolve the same service entry | Service ID, until a future ADR introduces a reviewed resource key. |
| Log output | Normalize the validated relative `steps[i].log` lexical path | Normalized case-sensitive relative filename. |

The injected value is not resolved, hashed, logged, or exported to the parent
process during preflight. Existing per-step checks still catch a duplicated
service entry and multiple injectors in one step. The new cross-step analysis
uses every unique service entry after those local errors are known.

### Conflict rules

1. **Unordered distinct-service injection collision.** If potentially
   concurrent steps reference different services whose `inject_env` is equal,
   emit one error. The primary path is the later deterministic step-service
   entry; related paths include the first step-service entry and both service
   `inject_env` paths. Help names the only valid repairs: create a dependency,
   use distinct injection names, or split the workflow.
2. **Unordered shared-service resource.** If potentially concurrent steps
   reference the same service ID, emit one error. The primary and related paths
   are the two step service entries, with the service definition as related
   context. This rule is emitted instead of a redundant injection-collision
   diagnostic for the same step pair and service ID. Help requires an explicit
   dependency or separate service resources.
3. **Duplicate log output.** If two distinct steps have equal normalized log
   identities, emit an error whether or not they are ordered. The primary and
   related paths are the two `steps[i].log` entries. Help requires unique
   `.log` filenames. The rule is global because serial reuse also overwrites a
   report artifact.

Each unordered pair/resource tuple produces at most one diagnostic of a given
rule. Choosing the lexicographically earlier step ID, then configuration index,
as the related side makes primary selection stable. Validation never repairs or
inserts relationships.

The future scheduler must consume the same reachability/resource semantics and
must retain a runtime lock as defense in depth. It may not treat two steps as
parallel-eligible when this preflight rejects their relation; any relaxation
requires a new ADR and OpenSpec change.

## 3. Future Report-Template Path Contract

This change introduces optional `[report_templates]` `root` and `template`
fields in the v2 schema. They declare safe, read-only template input only.
Before a later change enables a template or HTML renderer, it must preserve the
following invariants and define renderer-specific fields separately:

1. After interpolation and dedicated overrides, a template root and template
   file are non-empty, repository-relative lexical paths with no NUL, absolute
   path, parent component, or platform prefix.
2. The existing template root and target are canonicalized. Both remain under
   the canonical repository root, and the target remains under the canonical
   template root. A symlink that escapes either root is invalid.
3. The target is a regular file with an approved template extension. A
   directory, device, report file, or missing target is invalid.
4. The canonical template root and report-output root may not be equal and
   neither may be an ancestor of the other. Templates are read-only input;
   report output always uses the existing output policy.
5. Include, inheritance, and asset resolution use a loader confined to the
   same canonical template root. Top-level filename validation alone never
   authorizes arbitrary loader reads.

The implementation must define behavior for a non-existent path without using
canonicalization to bypass lexical checks. It may create output directories only
according to the current report-output policy; it must never create a template
path as a validation side effect.

## 4. Documentation, Migration, and Example Evidence

The final delivery updates the configuration reference with both editor
association and semantic-validation boundaries:

| Audience | Required information |
| --- | --- |
| VS Code Even Better TOML | Tested local association of `.harness-gate/flow.toml` to committed `schema/flow.schema.json`; no remote URL required. |
| Taplo | Tested local schema association with the same committed schema. |
| All editors | Schema validates structural shape; `harness-gate config check` validates interpolation, overrides, repository paths, references, and resource safety. |
| v1 users | Run `config migrate`, review emitted v2 TOML, provide required environment names, then run `config check`. |
| v2 users | No version bump; newly unsafe configuration must follow rendered paths/help and is never silently rewritten. |

Examples must be linked fixtures or extracted by the existing documentation
consistency mechanism. The inventory contains valid interpolation, ordered
shared service, distinct logs, and editor association examples; it also
contains negative fixtures for missing interpolation, unordered distinct-service
same injection, unordered shared service, duplicate log, lexical template
escape, and symlink template escape where supported.

## Alternatives Considered

### Fail fast with only human prose

Rejected. It makes editor/CI use fragile and hides the related fields that
explain a conflict.

### Only validate resources in the future scheduler

Rejected. A race could then be discovered after selection or service startup.
Resource safety is a static configuration property.

### Assume profiles or serial execution prevent overlap

Rejected. That assumption is invalid for `--all`, scope selection, and a
future parallel scheduler. Only explicit transitive ordering suppresses the
concurrency-specific diagnostics.

### Resolve template paths lazily inside a template engine

Rejected. An unrestricted include/inheritance loader would bypass top-level
path checks and expand filesystem access before containment is verified.

### Auto-repair conflicts

Rejected. Adding a dependency or changing an environment name modifies
execution/external behavior and requires author review.

## Rollback Plan

Because this OpenSpec is specification only, the immediate rollback is removal
of this unimplemented change directory. After implementation, rollback must
preserve existing files and reports, revert the validation/renderer surface as
one reviewed compatibility change, retain published diagnostic evidence, and
update the ADR/OpenSpec status. It must not automatically restore a
configuration that has already been explicitly migrated or modified by a user.

## Acceptance Criteria for This OpenSpec

- Proposal, design, tasks, and configuration-safety requirement delta exist in
  one named change directory.
- The diagnostic envelope, path grammar, ordering, confidentiality rules,
  resource relation, and template containment requirements are unambiguous.
- The change distinguishes static preflight from future scheduling and template
  rendering, and does not authorize either implementation.
- Every implementation task is under four hours, has a reviewable acceptance
  criterion, and is checked only when its linked evidence is reviewed.
- No scheduler, renderer, Tera loader, or business workflow behavior is
  included in this change; all new validation remains pre-execution.
