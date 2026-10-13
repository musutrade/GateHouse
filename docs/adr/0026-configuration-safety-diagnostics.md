# ADR-0026: Add Configuration Safety Diagnostics and Future-Concurrency Preflight

## Status

**Accepted with renderer follow-up** (2026-08-29)

The same-service preflight is refined for guaranteed serial dispatch by
[ADR-0050](0050-serial-shared-service-ordering.md). Other safeguards remain unchanged.

## Context

ADR-0023 established the v2 JSON Schema and deterministic `${NAME}` / `${NAME:-default}`
interpolation. ADR-0024 added `depends_on` validation and stable topological
ordering, while deliberately leaving scheduling and service locking out of
scope. The current configuration validator already rejects malformed values,
unknown references, duplicate service injections within one step, and invalid
dependency graphs.

The remaining Phase 2 work is cross-cutting. A configuration author who gets a
validation error normally sees a prose error without a stable configuration
field path, a source location, a related conflicting field, or a safe repair.
The validator also cannot yet explain resource hazards that become possible
when independently ordered steps are later scheduled concurrently:

- different services can inject the same child-process environment variable;
- two potentially concurrent steps can manage the same service resource; and
- two steps can write the same log file.

In addition, the report directory is currently limited to fixed JSON and
Markdown outputs. HTML reports will need a template-path configuration, but a
generic path accepted now could later permit a template loader to read outside
the repository or its approved template root. Finally, the generated schema is
useful only when editors, migration guidance, and verified examples tell users
how to consume it.

These concerns must be resolved before a parallel scheduler or an HTML/Tera
renderer is introduced. Deferring them would make a later scheduler choose
semantics implicitly, or turn an operational configuration mistake into a
runtime race or an unhelpful failure after a service has started.

## Decision

Treat configuration diagnostics, resource safety, and future report-template
inputs as one pre-execution validation boundary. This ADR extends the v2
configuration contract; it does not introduce parallel execution, an HTML
report, Tera, a template renderer, or a container-runtime abstraction.

### 1. Structured, field-addressable diagnostics

All configuration loading and validation failures will be represented internally
as diagnostics, then rendered by the CLI. A diagnostic has the following
stable data:

| Field | Meaning |
| --- | --- |
| `id` | Stable diagnostic identifier such as `HGCFG-DEPENDENCY-CYCLE`; it identifies the rule, not the wording. |
| `severity` | `error` for a rejected configuration; `warning` only for an explicitly documented migration advisory. |
| `path` | Canonical configuration field path. Array elements use zero-based brackets, map keys use escaped quoted brackets when needed (for example `steps[2].services[0]` and `services[\"test-db\"].inject_env`). |
| `message` | Concise reason, safe to print and free of secret values. |
| `help` | Actionable, deterministic remediation. |
| `location` | When source text is available, one-based line and column plus a bounded source span. |
| `related` | Zero or more paths/locations participating in the same conflict. |

The human renderer will show the configuration file, primary path, reason,
source location when known, related locations, and a `help:` line. It must not
render environment values, service connection strings, or template contents.
The machine renderer will expose the same fields as JSON so editors and CI do
not parse prose. Diagnostics are sorted by source order when locations exist,
otherwise by canonical path and `id`, and at most 50 are emitted in one run to
keep a malformed file usable. Parsing/interpolation failures that prevent a
complete source map remain single primary diagnostics but still include their
best-known location and a repair.

The command continues to fail before project discovery, service startup,
subprocess creation, or report writing if any error diagnostic exists. The
existing CLI exit-status convention remains unchanged. The existing outer
`E1000` command category remains valid for this boundary; diagnostic `id`s are
the new stable, field-level classification and are not a replacement public
exit code. Changes to the established error headline, exit status, or
machine-output format require an explicit snapshot review under ADR-0025.

Implement the diagnostics at the loader/validator boundary rather than by
matching `anyhow` display strings. TOML decoding and interpolation diagnostics
must retain source spans. Semantic validators must receive a source map keyed
by canonical field path so they can identify both sides of a conflict. Tests
and in-memory loading without a file may omit line/column, but must retain the
same `id`, path, message, help, and related paths.

### 2. Conservative preflight for service injections and future parallelism

Validation will derive a *potential-concurrency graph* from the configured
steps. Two distinct steps are potentially concurrent when neither is reachable
from the other through `depends_on`. This deliberately includes `--all` and
all-scope execution; profile or scope coincidence is not used to suppress a
safety finding unless a future execution model supplies a formally verified
mutual-exclusion rule.

For every step, validation derives these declarations without reading process
environment values or starting services:

- each referenced service ID and its injected environment-variable name;
- each service resource identity (the service ID until a later ADR introduces
  a distinct, reviewed resource key); and
- the normalized relative log filename.

The following are errors before execution:

| Condition | Primary and related paths | Required repair guidance |
| --- | --- | --- |
| One step lists two services that inject the same environment variable | The second `steps[i].services[j]` and the first conflicting entry | Use one service, change one service's `inject_env`, or remove the duplicate. |
| Potentially concurrent steps use different services that inject the same variable | The later step's service entry and both service `inject_env` fields | Add an explicit dependency, give the services distinct injection names, or split the intended workflow. |
| Potentially concurrent steps use the same service resource | Both step service entries and the shared service definition | Add a dependency or define separate service resources; do not rely on incidental startup order. |
| Any two selected-capable steps use the same normalized log filename | Both `steps[*].log` fields | Give each step a unique `.log` filename. |

The log rule is global rather than merely concurrent: serial reuse would still
overwrite a report artifact and make diagnostics or later parallel execution
ambiguous. The service-resource and injection rules are based on potential
concurrency and therefore do not reject a pair with an explicit transitive
ordering. Validation never inserts a dependency or silently renames an
environment variable or log file.

The future scheduler must consume this same graph and must not broaden the
definition of parallel eligibility without a new ADR. A runtime resource lock
is defense in depth, not an alternative to preflight validation. At this
stage, a service's injected environment remains task-local; no injected value
is exported into the parent process, persisted in a report, or logged by the
diagnostic renderer.

### 3. Report-template path safety contract

Add an optional v2 `[report_templates]` section with `root` and `template`
fields for the later HTML renderer. The fields declare read-only input only;
rendering selection and any other renderer options remain subject to a separate
renderer ADR. Every configured template path must obey this validation contract
from its first release:

1. A template path is a non-empty repository-relative path; it cannot be
   absolute, contain `..`, a platform prefix, or a NUL byte.
2. After interpolation and dedicated overrides, the lexical path and its
   canonical existing target must remain below the configured template root,
   which itself must resolve below the repository root. Symlinks may not escape
   either root.
3. The configured target must be a regular file with an approved template
   extension. Directories, device files, and report output paths are invalid.
4. The template root is read-only input and may not be the report-output
   directory or an ancestor/descendant of it. The renderer will write only to
   the existing report-output policy path, never next to a template.
5. Tera `include`, inheritance, and asset lookup must use a loader confined to
   that same canonical template root. Validation of the top-level filename is
   not sufficient to permit an unrestricted loader.

No configuration may opt into an HTML renderer or template rendering before
that renderer, its schema fields, and the confined-loader tests exist. Existing
JSON and Markdown report locations and names are unchanged.

### 4. Editor, migration, and example contract

The configuration documentation will name the committed
`schema/flow.schema.json` as the authoritative editor schema and include tested
instructions for at least the VS Code Even Better TOML and Taplo integrations.
Instructions must associate the schema with `.harness-gate/flow.toml` without
requiring a remote URL. They must also explain that schema validation covers
shape while `harness-gate config check` performs interpolation, path,
cross-reference, and resource-safety validation.

Migration guidance will distinguish these cases:

- v1 users run the documented `config migrate` path, review the emitted v2
  file, then run `config check` with required environment names available;
- existing valid v2 files require no schema-version bump; and
- a v2 file newly rejected by the safety rules receives both conflicting paths
  and one of the explicit repairs above. It is not automatically rewritten.

Checked-in examples must include valid configurations for interpolation,
ordered reuse of a service, unique logs, and an editor schema association. They
must also include negative fixtures for a missing environment variable,
duplicate cross-step injection, unordered shared service, duplicate log, and
template-root escape. Documentation snippets must link to, or be mechanically
checked against, those fixtures. The documentation-consistency gate from
ADR-0025 validates the schema, examples, migration input/output, links, and
the documented commands.

### 5. Validation and compatibility requirements

The implementation is complete only when all of the following are true:

1. `config check` produces deterministic human diagnostics and a documented
   machine-readable form for parse, interpolation, semantic, and multi-field
   resource conflicts.
2. Each diagnostic has a canonical path, stable ID, actionable help, and no
   secret disclosure; file-based inputs also report correct source locations
   for representative TOML, interpolation, and semantic errors.
3. Unit and integration tests cover direct, transitive, and unrelated
   dependency pairs; different/same injection names; same/different services;
   duplicate logs; Windows-style absolute/prefix path attempts; `..`; symlink
   escape; and a safe template path.
4. Existing valid presets, existing serial execution behaviour, report names,
   Schema export, interpolation precedence, and `depends_on` ordering remain
   compatible. The CLI/report/error snapshot suite is updated only through a
   reviewed, intentional contract change.
5. The editor instructions, migration guide, configuration reference, and
   examples are synchronized with the generated schema and pass the blocking
   documentation consistency job.

## Implementation Evidence and Follow-up

Typed diagnostics, JSON rendering, dependency/resource preflight, template
path containment, and their tests were implemented in
[PR #34](https://github.com/musutrade/Harness-Gate/pull/34). Project-scoped
audit selection was subsequently corrected in
[PR #38](https://github.com/musutrade/Harness-Gate/pull/38), whose clean
cross-platform quality run is [33217867592](https://github.com/musutrade/Harness-Gate/actions/runs/33217867592).

The existing report-template rendering surface is deliberately tracked by the
separate [confined report-template rendering proposal](../../openspec/changes/report-template-renderer/proposal.md).
That proposal must settle the schema, migration, compatibility, and loader
decisions before additional renderer-specific fields are added. The closeout
review and green CI evidence are recorded in
[PR #39](https://github.com/musutrade/Harness-Gate/pull/39).

## Consequences

### Positive

- Configuration authors receive all safely collectable defects at once, with
  exact fields and concrete repairs instead of a sequence of opaque failures.
- The configuration becomes a reliable input to a future scheduler: unsafe
  service and log relationships fail before external resources are created.
- Template paths have an explicit containment policy before HTML/Tera support
  makes unsafe filesystem loading part of the public configuration surface.
- Editors, migration users, local CLI validation, and CI share the same schema
  and semantic rules.

### Negative

- Retaining source maps and aggregating diagnostics increases parser/validator
  complexity and requires path-format tests as a compatibility surface.
- Some previously loadable v2 configurations with ambiguous future-concurrency
  resources will require explicit dependencies or distinct names, even while
  execution remains serial.
- Canonical path and symlink checks are platform-sensitive and require focused
  Windows and Unix test coverage.
- The template policy intentionally constrains a later renderer and may require
  a migration if it needs a more expressive, separately reviewed asset model.

## Alternatives Considered

### Keep prose-only, fail-fast errors

Rejected. They are difficult for editors and CI to consume and force users to
repair complex configurations one failure at a time. They also obscure the
second side of a resource conflict.

### Wait for the parallel scheduler to validate resources

Rejected. The scheduler would discover conflicts after plan selection and
possibly after starting a service. The safety relation belongs to the static
configuration graph and must be established before execution.

### Treat profiles or current serial execution as proof that steps cannot overlap

Rejected. `--all`, scope selection, and a future scheduler make that assumption
unsound. A future mutual-exclusion feature may narrow the graph only if its
semantics are formally specified and tested.

### Permit arbitrary template file paths until HTML reports exist

Rejected. It postpones an input-containment decision until a template engine
has already expanded the filesystem attack surface. A confined root and loader
are prerequisites for enabling rendering.

### Automatically repair conflicts by adding dependencies or renaming values

Rejected. Either action changes execution order or external process contracts
without the configuration author's review. Diagnostics must explain the
conflict; the author chooses the repair.

## References

- [ADR-0023: Generate configuration schema and interpolate environment variables](0023-config-schema-and-interpolation.md)
- [ADR-0024: Add dependency ordering to verification steps](0024-verification-plan-dependencies.md)
- [ADR-0025: Establish Phase 1 quality baseline gates](0025-phase-1-quality-baseline-gates.md)
- [Configuration reference](../configuration.md)
