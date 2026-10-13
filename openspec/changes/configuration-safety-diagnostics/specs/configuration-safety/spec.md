# Configuration Safety Specification

## ADDED Requirements

### Requirement: Field-addressable configuration diagnostics

The system SHALL render every configuration parsing, interpolation, semantic,
dependency, and resource-preflight failure as a typed diagnostic before any
project discovery, service operation, subprocess execution, report write, or
scheduler operation begins. Each diagnostic SHALL expose a stable `HGCFG-*`
identifier, severity, canonical field path, safe message, actionable help, and
zero or more related fields. File-backed inputs SHALL include a one-based
source location when the relevant source span is available.

The system SHALL NOT render resolved environment-variable values, service
connection values, or template contents in diagnostics, related fields, JSON,
or human output.

#### Scenario: Semantic field error includes a safe repair

- **WHEN** `steps[2].timeout_secs` is outside its accepted range
- **THEN** validation fails with an `error` diagnostic whose primary path is
  `steps[2].timeout_secs`
- **AND THEN** the diagnostic has a stable `HGCFG-*` ID and help describing the
  accepted range
- **AND THEN** its message contains no unrelated configuration or environment
  values

#### Scenario: Multi-field conflict identifies both sides

- **WHEN** two independently ordered steps create a service-injection conflict
- **THEN** validation emits one error diagnostic with a primary step-service
  path
- **AND THEN** its related entries identify the conflicting step-service entry
  and the relevant service injection-field paths
- **AND THEN** help names an explicit dependency or distinct injection names as
  repair options

#### Scenario: Source-less in-memory validation remains actionable

- **WHEN** an in-memory configuration source has a semantic validation error
- **THEN** its diagnostic may omit a location
- **BUT THEN** it still contains ID, severity, canonical path, message, help,
  and applicable related paths

### Requirement: Deterministic aggregate diagnostics

The system SHALL collect independently determinable diagnostics in one config
check, up to 50 entries. It SHALL stop a phase only when a prior error makes
further claims unreliable. Diagnostics with locations SHALL sort by source
position, path, and ID; diagnostics without locations SHALL sort by path and
ID. Related entries SHALL sort deterministically. The JSON output SHALL mark a
result as truncated and include a deterministic truncation diagnostic if the
limit is reached.

#### Scenario: Independent errors are reported together

- **WHEN** a configuration contains an invalid step timeout and an unknown
  parser reference in independently valid TOML fields
- **THEN** one config check reports both diagnostics in deterministic order
- **AND THEN** correcting one does not change the identity/path/help of the
  other

#### Scenario: Syntax failure prevents unsafe semantic claims

- **WHEN** TOML syntax is malformed before a semantic field can be decoded
- **THEN** config check reports the parser diagnostic with its best-known
  location and repair
- **AND THEN** it does not claim semantic validation of the undecodable portion
  succeeded or failed

### Requirement: Machine-readable configuration check output

The CLI SHALL provide `harness-gate config check --format json`. Its stdout
SHALL be exactly one version-1 JSON diagnostic envelope with `schema_version`,
`valid`, `truncated`, and `diagnostics`. Its stderr SHALL contain no human
diagnostic prose. A failed JSON config check SHALL preserve the existing
nonzero failure status/error-category convention, and a valid JSON config
check SHALL exit successfully with an empty diagnostic array.

The default human `harness-gate config check` interface SHALL retain its
established success output, top-level error category, and exit convention
unless a separately reviewed public snapshot change says otherwise.

#### Scenario: Invalid configuration yields parseable JSON

- **WHEN** a user runs `config check --format json` against a configuration
  with a missing interpolation variable
- **THEN** stdout parses as one version-1 envelope with `valid: false`
- **AND THEN** its diagnostic identifies the interpolation failure without
  exposing the variable's value
- **AND THEN** stderr has no human-rendered error text and the command exits
  nonzero

#### Scenario: Valid configuration yields an empty diagnostic set

- **WHEN** a user runs `config check --format json` against a valid v2 file
- **THEN** stdout contains `valid: true`, `truncated: false`, and an empty
  `diagnostics` array
- **AND THEN** the command exits zero

### Requirement: Conservative potential-concurrency analysis

The system SHALL treat two distinct configured steps as potentially concurrent
when neither step transitively depends on the other. Profile selection, scope,
and current serial execution SHALL NOT suppress that classification. The system
SHALL perform dependency validity checks before resource analysis and SHALL use
the same relation as the future scheduler eligibility boundary.

#### Scenario: Transitive dependency prevents a concurrency finding

- **WHEN** step `c` depends on `b` and `b` depends on `a`
- **THEN** `a` and `c` are not potentially concurrent
- **AND THEN** a shared-service or distinct-service injection relation between
  only those two steps does not produce an unordered-concurrency diagnostic

#### Scenario: Unrelated profiles do not mask a possible race

- **WHEN** two steps have no dependency path between them and currently belong
  to different profiles
- **THEN** they remain potentially concurrent for preflight purposes

### Requirement: Service injection and resource conflict preflight

The system SHALL reject one step that references multiple services with the
same injected environment-variable name. It SHALL also reject potentially
concurrent steps that reference different services with the same injection
name, and potentially concurrent steps that reference the same service
resource. A shared-service finding SHALL take precedence over a redundant
same-service injection finding for the same step pair.

The system SHALL derive only service IDs and injection names; it SHALL NOT read,
persist, hash, export, or render injected service values during preflight. It
SHALL NOT insert dependencies or rename environment variables automatically.

#### Scenario: Ordered service reuse is permitted

- **WHEN** step `integration` depends directly or transitively on step `setup`
- **AND WHEN** both reference the same service
- **THEN** the shared service relation does not fail the potential-concurrency
  preflight

#### Scenario: Unordered different services collide on injection name

- **WHEN** two potentially concurrent steps reference different services that
  both inject `TEST_DATABASE_URL`
- **THEN** validation fails before execution
- **AND THEN** diagnostics identify both steps and both service injection fields
- **AND THEN** help recommends an ordering dependency or distinct names

#### Scenario: Unordered shared service is rejected

- **WHEN** two potentially concurrent steps reference the same service ID
- **THEN** validation fails before execution with a shared-service-resource
  diagnostic
- **AND THEN** the result does not emit a redundant same-service injection
  collision for that step pair

### Requirement: Unique verification log outputs

The system SHALL normalize each valid relative step log path and SHALL reject
the same normalized log identity in two distinct configured steps, regardless
of dependency ordering. The diagnostic SHALL identify both `steps[*].log`
paths and instruct the author to choose unique `.log` filenames.

#### Scenario: Ordered steps cannot overwrite a log

- **WHEN** step `test` depends on step `lint`
- **AND WHEN** both configure `verification.log`
- **THEN** configuration validation fails with a duplicate-log diagnostic
- **AND THEN** no serial execution begins

### Requirement: Future report-template path containment

The optional `[report_templates]` root and target validation SHALL reject empty
paths, NUL, absolute paths, platform prefixes, parent traversal, missing or
non-regular targets, canonical root escape, and symlink escape. It SHALL
require the template target to remain inside a canonical repository-contained
template root, and that root SHALL be disjoint from the report-output root.

The future template loader SHALL confine top-level loading, include,
inheritance, and asset lookup to that canonical template root. Validation SHALL
NOT create template paths or write report files beside templates.

#### Scenario: Symlinked template cannot escape the approved root

- **WHEN** a repository-contained template path resolves through a symlink to a
  location outside its configured template root
- **THEN** template validation rejects the configuration before rendering
- **AND THEN** the diagnostic identifies the template path and containment rule

#### Scenario: Template and report roots may not overlap

- **WHEN** a configured template root equals, contains, or is contained by the
  configured report-output root
- **THEN** template validation rejects the configuration
- **AND THEN** no renderer writes to either location

### Requirement: Editor, migration, and example consistency

The documentation SHALL provide tested local schema-association instructions
for VS Code Even Better TOML and Taplo using committed
`schema/flow.schema.json`. It SHALL state that schema validation is structural
and that `config check` performs interpolation, override, containment,
cross-reference, and resource-safety validation.

Migration guidance SHALL describe v1 migration and config checking, preserve
the v2 version number for existing files, and require authors to repair newly
unsafe v2 resource relations explicitly. Valid and negative examples SHALL be
linked or mechanically extracted and SHALL be checked by the ADR-0025
documentation-consistency gate.

#### Scenario: Editor user receives the complete validation boundary

- **WHEN** a user follows the documented local editor schema setup
- **THEN** the editor can load the committed schema for `.harness-gate/flow.toml`
- **AND THEN** documentation directs the user to `config check` for rules the
  schema cannot evaluate

#### Scenario: Migration does not silently alter ordering

- **WHEN** a v2 configuration is newly rejected for unordered service reuse
- **THEN** migration guidance presents the conflicting paths and repair choices
- **AND THEN** no tool silently inserts `depends_on` or renames a service/log
