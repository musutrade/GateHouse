# harness-gate schema v2 configuration reference

The primary workflow is **`init -> verify -> one project decision`**. Start with
the [quality quick start and architecture](quality-workflow.md), including
capability states, certification, baseline/profile behavior, artifact trust and
troubleshooting. Execution mechanics below compose with quality policy;
`quality evaluate` and `adapter run` are advanced interfaces.

For generated quality defaults, composable packs and flow-only adoption, see
[quality presets and migration](quality-presets.md).

For compatible Arc-Flow v2 execution declarations, `harness-gate config import --execution-only`
generates configuration and a deterministic parity/UX report. See
[Arc-Admin import and runtime limitations](dogfood/arc-admin/import/README.md).
Legacy external service policies must first be migrated by GateProbes. The
offline importer preserves explicit generic `service_checks` and their required
artifact bindings without executing them; it refuses legacy policies with
migration help and publishes no partial configuration.

For optional `.harness-gate/quality.toml` v1, collector authority and flow/quality
cross-validation, see the [quality configuration reference](quality-configuration.md).
Repositories without that file retain existing flow-only behavior.


This is the English reference for .harness-gate/flow.toml, secret scanning,
and architecture auditing. The [JSON Schema catalog](../schema/README.md)
lists every machine-readable contract. The Chinese reference is
[configuration.zh-CN.md](configuration.zh-CN.md).

Harness-Gate is data-driven: projects add commands, components, profiles,
parsers, and services in TOML without changing the Rust engine. JSON Schema
checks structure; config check also checks interpolation, paths, references,
and resource safety.

## 1. Loading and commands

The default file is .harness-gate/flow.toml. The CLI searches from the
current directory toward its parents, or accepts explicit paths:

    harness-gate --project-root /path/to/project config check
    harness-gate --project-root /path/to/project \
      --config config/ci-flow.toml config check
    harness-gate config check --format json
    harness-gate config print --resolved
    harness-gate schema export

The project root and configuration must stay inside the checkout. Absolute
paths, .. traversal, and symlinks that resolve outside the project are
rejected. `${NAME}` requires an existing variable; `${NAME:-default}` supplies
a default. Interpolation runs before TOML parsing and is not recursive.
paths.audit_config must instead be a literal repository-relative path so an
external environment cannot replace the audit policy.

config check runs before services, child processes, or reports are created.
JSON diagnostics contain stable HGCFG-* IDs, field paths, a reason, and a
remediation hint. Interpolated secrets, connection strings, and template
contents are not emitted.

## 2. Editor schema association

schema/flow.schema.json is the local schema for .harness-gate/flow.toml.
Run harness-gate schema export whenever the Rust configuration model changes;
CI checks that the generated file is committed.

VS Code with Even Better TOML:

    {
      "evenBetterToml.schema.associations": {
        "./schema/flow.schema.json": [".harness-gate/flow.toml"]
      }
    }

Taplo:

    [[schema.associations]]
    url = "./schema/flow.schema.json"
    include = [".harness-gate/flow.toml"]

Editors catch structural errors first. Run config check --format json for
runtime-aware diagnostics.

## 3. Names, paths, and placeholders

IDs for projects, components, profiles, steps, parsers, services, doctor
checks, and aliases must be non-empty lowercase ASCII names containing only
letters, digits, ., -, and _. Environment variable names use uppercase
letters, digits, and _. A program is a bare executable from PATH; it
cannot contain / or a backslash.

Repository paths are non-empty relative paths without parent traversal.

| Placeholder | Meaning |
| --- | --- |
| {root} | Absolute project root. |
| {reports} | Absolute report directory. |
| {audit_config} | Absolute audit rules path. |
| {<alias>} | Absolute path from [paths.aliases.<alias>]. |
| {host_port} | Random host port allocated for a managed service. |

A step cwd must be exactly {root} or one declared alias. Declare an alias for
a subdirectory instead of adding a suffix to cwd.

## 4. Top-level structure

    version = 2

    [project]
    # ...

    [paths]
    # ...

    [policy]
    # ...

    [execution]
    # parallel = false
    # max_parallel = 4

    [notifications]
    # [[notifications.webhooks]]
    # ...

    [[doctor.checks]]
    # ...

    [services.example]
    # ...

    [parsers.example]
    # ...

    [scope]
    # unmatched = "fail"

    [[scope.rules]]
    # ...

    [[steps]]
    # ...

| Section | Required | Purpose |
| --- | --- | --- |
| version | Yes | Must be 2. |
| [project] | Yes | Project ID and default profiles. |
| [paths] | Yes | Reports, audit rules, and aliases. |
| [policy] | No | Required steps and expiring waivers. |
| [execution] | No | Dependency-aware scheduling and retries. |
| [doctor] | No | Local environment checks. |
| [services.*] | No | Environment or managed container services. |
| [parsers.*] | No | Test-result parsing. |
| [scope] | Yes | Changed-path matching behavior. |
| [[steps]] | Yes | External commands or built-in gates. |
| [report_templates] | No | Optional HTML and JUnit output. |
| [notifications] | No | Optional HTTP(S) notifications. |

Unknown fields fail parsing rather than being silently ignored.

## 5. Project and paths

    [project]
    name = "orders-api"
    default_profile = "full"
    hook_profile = "hook"

    [paths]
    reports = ".harness-gate/reports"
    audit_config = ".harness-gate/audit.toml"
    secrets_config = ".harness-gate/secrets.toml"

    [paths.aliases.api]
    path = "services/api"
    env = "HARNESS_GATE_API_DIR"

project.name, default_profile, and hook_profile are required IDs. Profiles are
created by the profiles set on steps, and both project profiles must be
referenced by at least one step.

paths.reports holds JSON, Markdown, optional HTML/JUnit output, and invocation
evidence. paths.audit_config points to audit rules. paths.secrets_config
defaults to .harness-gate/secrets.toml. Alias env may override an alias path
for one invocation. root, reports, audit_config, and host_port are reserved
placeholder names.

REPORT_DIR (compatibility alias, warning until 2027-03-01) or
HARNESS_GATE_REPORTS overrides paths.reports; HARNESS_GATE_SECRETS_CONFIG
overrides paths.secrets_config. PROJECT_ROOT, HARNESS_GATE_CONFIG,
AUDITOR_CONFIG, and HARNESS_GATE_AUDIT_CONFIG do not participate in discovery.
Select a project explicitly with --project-root and --config.

### Invocation inputs

Each invocation records an input mode, project identity, source identity,
execution root, and configuration digest. Ordinary verify reads the working
tree. scope --staged and hook materialize the complete Git index into a private
temporary snapshot, run every gate there, and remove it afterwards.

Steps default to input = "snapshot" ("staged" is a compatibility alias):

| Input | Behavior |
| --- | --- |
| snapshot | Read from the immutable invocation root; hooks use the complete staged snapshot. |
| repository | Read the original checkout. This is an explicit compatibility capability for commands that need Git metadata. |

Secret scan, architecture audit, and scope always use the invocation input.
Machine results include input_mode, source_identity, execution_root, and
configuration_digest.

## 6. Execution, retries, and shards

    [execution]
    parallel = true
    max_parallel = 4

    [execution.retries.api.tests]
    max_attempts = 3
    backoff_ms = 250
    retryable = ["timeout", "exit"]

    [execution.shards.api.tests]
    index = 0
    total = 2

parallel defaults to false; max_parallel defaults to 4 when enabled and must be
1..=64. Dependencies, service locks, and the built-in secret/audit order remain
authoritative. Results are printed in stable plan order. A failed dependency
prevents its descendants from running; reports list them in skipped_steps and
keep the overall result failed.

Retry policies are keyed by step ID. max_attempts is 1..=5; retryable is the
closed set cancelled, timeout, parser, and exit. Results include every attempt,
retry count, flaky state, parser completeness, and shard identity.

### Versioned runner contract

    [[steps]]
    id = "backend.tests"
    label = "backend tests"
    component = "backend"
    profiles = ["full"]
    program = "cargo"
    args = ["test", "--manifest-path", "backend/Cargo.toml"]
    cwd = "{root}"
    log = "backend_tests.log"
    timeout_secs = 300
    runner = { version = 2, kind = "cargo-test", threads = 4,
               required_program = "cargo",
               threads_args = ["--test-threads", "{threads}"], threads_args_after = "--",
               result_format = "junit", isolation = "schema-per-worker" }

| Field | Required | Constraints |
| --- | --- | --- |
| version | Yes | Runner contract version, currently 2; v1 is rejected with a migration diagnostic. |
| kind | Yes | Opaque runner ID; command behavior comes from declarations. |
| threads | No | 1..=256; the concurrency budget. |
| threads_env | No | Thread-count variable; requires threads. |
| required_program | No | Require this exact step executable. |
| threads_args | No | Thread argument templates; requires threads and at least one `{threads}` placeholder. Other placeholders are rejected. |
| threads_args_after | No | Literal token after which to insert thread arguments; append token if absent. Requires threads_args. Omitted means append arguments. |
| args | No | Runner-specific arguments. |
| args_position | No | Zero-based insertion index; omitted means append. |
| result_format | No | regex, junit, trx, or json; default regex. |
| isolation | Yes | shared, schema-per-worker, or database-per-worker. |

Arguments are composed in order: insert `args` at `args_position`, then insert
expanded `threads_args` after the first `threads_args_after` token, or append
when no token is declared. Duplicate user flags stay in their original order;
the invoked runner retains authority over their command-line conflict behavior.
`threads > 1` requires `threads_env` or `threads_args`, and cannot use shared isolation.

Runner v1 must be explicitly migrated with GateProbes
[`tools/runners/migrate.py`](https://github.com/musutrade/GateProbes/blob/main/tools/runners/migrate.py).
It consumes the external Cargo definition and writes a new flow, preserving
policy, parser, isolation, shard and retry values. Run `config check` with the new
Core before replacement. Keep the original flow and binary together for rollback;
v2 declarations are incompatible with Core releases supporting only runner v1.
The overall flow version remains 2. See [ADR 0054](adr/0054-declarative-runner-inputs.md).

 With global parallelism, external
steps must declare runner isolation. Worker identity is passed through reserved
HARNESS_GATE_ISOLATION_* variables and cleaned up after the invocation.

## 7. Policy and waivers

    [policy]
    required_steps = ["api.format", "api.tests"]

    [[policy.waivers]]
    id = "incident-123"
    step = "api.tests"
    scope = "api"
    risk = "medium"
    reason = "upstream fixture unavailable"
    owner = "team-api"
    approved_by = "security-reviewer"
    created_at = "2026-08-30T00:00:00Z"
    expires_at = "2026-09-02T00:00:00Z"
    compensating_control = "manual smoke test attached to incident"

Every required_steps ID must exist exactly once in [[steps]]. Waivers require an
owner, approver, reason, compensating control, and RFC3339 creation and expiry
timestamps. Expired, revoked, out-of-scope, or self-approved waivers fail
before execution. A valid waiver is reported as WAIVED, not PASS.

## 8. Doctor checks

    [[doctor.checks]]
    id = "tool.git"
    label = "git"
    required = true
    timeout_secs = 15
    help = "install Git and ensure it is on PATH"
    kind = "command"
    program = "git"
    args = ["--version"]

Common fields are id, label, required (default true), timeout_secs (1..=300,
default 15), and optional help.

| Kind | Fields | Behavior |
| --- | --- | --- |
| command | program, args | Command must exit 0. |
| path | path, path_type, path_scope | Check any path, file, or directory. |
| glob | pattern | Pattern must match at least one path. |
| env | name | Variable must exist. |
| env-or-file | env, path, contains, path_scope | Variable exists, or a file has a line beginning with contains. |
| git-config | key, expected | Git value must equal expected. |
| git-remotes | none | Validate Git remote configuration. |
| version | program, args, path, trim_prefix, path_scope | Compare command output with a version file. |
| service | service | Check an environment or managed service. |

path_type is any, file, or directory and defaults to any. path_scope is
repository (default) or host. Repository-scoped paths must stay inside the
project: a repository-relative path, optionally behind one leading placeholder
such as {root} or an alias, with no `..` and no symlink that resolves outside.
config check and doctor both reject other paths. Set path_scope = "host" only
for an intended host location such as an installed tool or a user-level
credential file; reports name the path but never echo file content. Use
harness-gate doctor --strict in CI when warnings must fail the job.

### Lease and orphan cleanup

Managed Docker/Podman services and invocation directories use JSON leases under
<reports>/leases/. Lease schema 2 contains project identity, deterministic
resource ID and kind, invocation ID, process identity, creation/heartbeat/
expiry times, runtime labels, and the immutable runtime object ID.

A lease is a resource claim, not delete authorization. Cleanup re-inspects a
container and compares filename, project, resource, invocation, ownership
labels, and immutable object ID. Missing or changed evidence, inspect errors,
old schemas, and cross-project records leave resources untouched.

    # Observe only; do not stop containers or delete leases.
    harness-gate cleanup --dry-run --json

    # Reclaim dead or expired Harness-Gate leases.
    harness-gate cleanup --json

Cleanup writes cleanup.json. Unknown markers, malformed records, and active
owners are never reclaimed; a stop failure leaves the lease for a later retry.

## 9. Services

Environment services forward values supplied by CI or a secret manager:

    [services.test-redis]
    kind = "environment"
    source_env = "CI_REDIS_URL"
    inject_env = "TEST_REDIS_URL"

Managed services use an existing local OCI image and never pull implicitly:

    [services.test-postgres]
    kind = "docker"
    runtime = "docker" # or podman
    image = "postgres:16-alpine"
    image_env = "HARNESS_GATE_POSTGRES_IMAGE"
    external_env = "TEST_DATABASE_URL"
    inject_env = "TEST_DATABASE_URL"
    startup_timeout_secs = 30
    timeout_env = "HARNESS_GATE_DATABASE_TIMEOUT_SECS"
    container_port = 5432
    environment = { POSTGRES_USER = "test", POSTGRES_PASSWORD = "test", POSTGRES_DB = "app_test" }
    healthcheck = ["pg_isready", "-U", "test", "-d", "app_test"]
    connection = "postgres://test:test@127.0.0.1:{host_port}/app_test"

runtime is docker by default or compatible podman. image, inject_env,
startup_timeout_secs, container_port, healthcheck, and connection are
required. image_env, external_env, timeout_env, and environment are optional.
Postgres URL isolation and health checks belong to the GateProbes
[Postgres service plugin](https://github.com/musutrade/GateProbes/tree/main/plugins/postgres-service).
Declare the installed artifact as a required check (replace the path and digest
with the actual verified installation):

    [[service_checks.test-postgres]]
    executable = "/opt/gateprobes/gateprobes-postgres"
    sha256 = "<64-character installed artifact SHA-256>"
    protocol = "gate-service-check/v1"
    plugin = "gateprobes-postgres"
    version = "0.1.1"
    timeout_secs = 15
    pass_env = ["DATABASE_URL", "HARNESS_GATE_ALLOW_REMOTE_TEST_DATABASE", "PATH", "SYSTEMROOT"]

Every declared check is required; an empty check list is invalid. Core runs the
artifact in the project root with a cleared environment, forwarding only
`pass_env` and injecting `GATE_CHECK_PROTOCOL`, `GATE_CHECK_MODE` (`external` or
`managed`) and, for an external service, `GATE_CHECK_VALUE`. Values never enter
argv or published plugin diagnostics. Core checks the executable digest before
and after execution, then validates a bounded JSON result containing the exact
protocol, plugin, version, mode and `success = true`. Missing or modified
artifacts, incompatible identity, malformed/missing results, failure, crash and
timeout block the dependent step. Doctor uses the same service preparation and
checks; include a required service doctor check. Managed container health uses
the explicit recipe above; external Postgres health requires `pg_isready` on PATH.

Core retains requiredness, process execution, service lifecycle, leases, cleanup
and the final blocking decision. Plugin upgrades require explicit digest/version
rebinding and do not rebuild Core. Released Core v0.5.0 supports this contract; v0.4.9 is incompatible.
GateProbes package acceptance uses the independently verified released Core. Old
`external_value_policy` configurations are rejected with a migration diagnostic,
including `none`; removing the field alone does not preserve Postgres assurance.
Service injection names must be unique within a step and cannot collide with
runner thread variables or remove_env.

## 10. Parsers

    [parsers.rust]
    kind = "regex"
    patterns = ['(?m)^running ([0-9]+) tests?$']
    capture = 1
    minimum = 1

    [parsers.junit]
    kind = "junit"
    minimum = 1

    [parsers.dotnet]
    kind = "trx"
    minimum = 1

    [parsers.json-results]
    kind = "json"
    count_path = "results"
    minimum = 1

regex counts a capture group; junit reads JUnit XML; trx reads Visual Studio
TRX XML; and json reads an array or non-negative integer at `count_path`.
`count_path` is a non-empty dot-separated object path; it does not support
JSONPath expressions, array indexes, or wildcards. When `count_path` is omitted,
JSON keeps compatibility with a bare root array or exactly one recognized result
array. Recognized field names are `testcases`, `testCases`, `test_results`,
`testResults`, and `results`; object wrappers may surround the field, but result
arrays are not searched recursively.

Automatic discovery rejects arbitrary numbers (including `duration_ms`), unrelated
arrays such as metadata or attachments, non-array recognized fields, and documents
with multiple recognized candidates—even when their lengths are equal. These
inputs now fail with `RESULT_PARSE_FAILURE`; they are not counted as tests.
Producers that previously relied on implicit numeric discovery must add an explicit
path, for example:

```toml
[parsers.json-results]
kind = "json"
count_path = "summary.total"
minimum = 1
```

An explicit path is never retried through automatic discovery. Missing paths,
negative or fractional numbers, and values that are not arrays or non-negative
integers are parse failures. `minimum` defaults to 1. Empty, malformed, or
partial results fail closed as `RESULT_ZERO`, `RESULT_PARSE_FAILURE`, or
`RESULT_PARTIAL` respectively.

## 11. Scope rules

    [scope]
    unmatched = "fail" # fail, all, or ignore

    [[scope.rules]]
    patterns = ["services/api/**"]
    components = ["api"]

unmatched defaults to fail and rejects any changed path without a matching rule.
all selects every component only when at least one changed path is unmatched;
a clean tree selects none, and fully matched changes select only their mapped
components. ignore drops unmatched paths from component selection. Explicit
--all always selects every component, regardless of changed paths or this policy.
Rules map Git paths to component IDs and the selected set is recorded in the
machine result.

### Explicit component impact (optional)

```toml
[scope.impact]
version = 1
[scope.impact.dependencies]
shared = []
api = ["shared"]
web = ["api"]
unrelated = []
```

Every key must be an existing workflow component; declare every component, even
with no dependencies. `api = ["shared"]` means api consumes shared. A shared
change selects shared, api and web; a web change selects only web. Profiles still
filter each component's steps. Step `depends_on` remains execution prerequisites.
Explicit `--components` stays exact; `--all` and unmatched policies keep their
existing behavior. Rename/delete paths participate without reading removed files.

For generated declarations, add `[scope.impact.inputs]` with repository-relative
text paths as quoted keys and exact lowercase SHA-256 digests as values. Pin every
manifest/lockfile that can change the graph. Missing or stale pins block selection;
staged mode checks indexed bytes, including read-only previews. Core does not run
providers or trust plugin graph responses. Authors must review static declarations
and the completeness of pins; undeclared imports cannot be inferred.

Unknown versions, incomplete inventories, duplicate edges, cycles and unsafe
inputs fail closed. Limits are 1024 components, 8192 edges, 64 input pins and
4 MiB per input. `verify --plan` explains indirect selections with COMPONENT_IMPACT,
`impact_chain`, original `impact_sources` path/rule pairs and full
`depends_on_components` declarations. Existing configurations need no migration.
Older binaries reject the new field. To roll back, remove the impact section and
use `--all` or restore equivalent reviewed glob coverage before downgrading.
See the [design and cost model](../openspec/changes/component-impact-propagation/design.md).

## 12. Steps and built-in gates

    [[steps]]
    id = "api.tests"
    label = "API tests"
    component = "api"
    profiles = ["full", "ci"]
    program = "cargo"
    args = ["test", "--manifest-path", "services/api/Cargo.toml"]
    cwd = "{root}"
    log = "api_tests.log"
    timeout_secs = 300
    parser = "rust"
    services = ["test-postgres"]
    depends_on = ["api.format"]
    input = "snapshot"
    remove_env = ["UNSAFE_TEST_MODE"]

| Field | Required | Notes |
| --- | --- | --- |
| id | Yes | Globally unique step ID. |
| label | Yes | Human-readable report label. |
| component | Yes | Component selected by scope. |
| profiles | Yes | At least one profile. |
| program | Yes | Bare executable name. |
| args | Yes | Independent argument array; placeholders are expanded. |
| cwd | Yes | Exactly {root} or one alias. |
| log | Yes | Unique single .log name under reports. |
| timeout_secs | Yes | 1..=3600; may be overridden by timeout_env. |
| timeout_env | No | Environment variable for timeout override. |
| parser | No | Parser ID from [parsers.*]. |
| services | No | Service IDs prepared before the command. |
| remove_env | No | Inherited variables removed before spawn. |
| depends_on | No | Step IDs that must complete first. |
| input | No | snapshot (default) or explicit repository. |
| kind | No | builtin-gate or omitted for an external step. |
| gate_type | No | secret-scan or architecture-audit for built-in gates. |
| runner | No | Versioned runner contract. |

Commands are spawned as program + args[]; shell strings such as sh -c and
bash -lc are rejected. Split pipelines into separate steps or use a versioned
project executable. Built-in secret-scan and architecture-audit gates run before
external steps. Reserved built-in IDs cannot be replaced by an external step.

## 13. Secret scan rules

paths.secrets_config points to a separate, version-controlled TOML file:

    version = 2

    [placeholders]
    minimum_unique_characters = 4
    maximum_nonalphanumeric_characters = 2
    prefixes = ["\${", "{{", "<"]
    markers = ["change-me", "replace-me", "placeholder"]
    exact = ["password", "secret"]

    [[rules]]
    id = "named-signing-secret"
    kind = "value"
    pattern = '''(?i)signing_secret\s*=\s*([A-Za-z0-9_-]{12,})'''
    capture = 1
    minimum_length = 12

Rule kinds are direct, value, postgres-url, and webhook-url. Missing, empty,
or malformed rules fail before scanning. Reports contain filenames and
locations, never secret values. Regular files larger than 16 MiB, symlink
escapes, and non-regular inputs are handled by the scanner's closed policy.

## 14. Audit rules and v2 migration

paths.audit_config points to the architecture auditor's TOML file:

    version = 2

    [engine]
    ignore_filename = ".auditignore"
    json_report_filename = "review_context.json"
    markdown_report_filename = "review_context.md"
    markdown_max_bytes = 4096
    markdown_occurrences_per_rule = 3

    [paths]
    exclude = ["target", "node_modules", "dist", ".git"]

Each scanned extension needs a matching
[engine.comment_syntax.<extension>] definition. Rules use explicit
path-prefix or regex allowlists:

    [[hard_rules]]
    name = "SQL writes stay in repositories"
    severity = "blocker"
    paths = ["api"]
    extensions = ["rs", "sql"]
    patterns = ['(?i)INSERT\s+INTO']
    allowlist = [
      { kind = "path-prefix", path = "services/api/src/repositories" },
      { kind = "regex", pattern = '^services/api/generated/.*\.rs$' },
    ]
    exclude_patterns = []

Paths must resolve to existing in-project directories. Absolute paths, ..
traversal, symlink escapes, unreadable files, and files larger than 16 MiB fail
closed. The auditor is a deterministic whole-file regex scanner, not an AST
parser; use a language-specific linter for AST rules.

<a id="audit-v2-migration"></a>

### Audit v2 migration

Audit v2 is the breaking configuration format introduced with Harness-Gate
3.0.0. A legacy file is never silently assigned new semantics. Missing
version, missing [engine], unknown versions, unknown fields, or string
allowlists fail and point here.

Add version = 2, copy the complete [engine] and required comment syntax sections
from the empty preset, and convert each string allowlist explicitly:

    allowlist = [
      { kind = "path-prefix", path = "services/api/src/repositories" },
      { kind = "regex", pattern = '^services/api/generated/.*\.rs$' },
    ]

String allowlists cannot be migrated reliably because their intent is
ambiguous. After conversion run harness-gate config check and harness-gate
audit and confirm every path, regex, and report option.

## 15. Reports, templates, and notifications

    [report_templates]
    root = "templates/harness-gate"
    template = "templates/harness-gate/verification.tera"
    junit = "junit.xml"

Template root and template must be repository-contained regular paths with no
traversal or symlink escape, and the template must end in .html or .tera.
Template and report roots must be disjoint. HTML output is test_result.html;
JUnit is a report-relative .xml path. Tera supports include, extends, and block
plus timestamp, profile, scope, steps, summary, components, and passed.

Canonical evidence is written under
<reports>/invocations/<invocation_id>/, including invocation.json, scope.json,
step logs, and reports. Legacy test_result.json and test_result.md remain
mirrored at the report root. Their contracts are
[machine-result.schema.json](../schema/machine-result.schema.json),
[artifact-manifest.schema.json](../schema/artifact-manifest.schema.json), and
[artifact-registry.schema.json](../schema/artifact-registry.schema.json).
Artifacts are invocation-relative ordinary files with kind, byte count, and
SHA-256. Missing, extra, outside, replaced, or symlinked artifacts make
evidence_complete false.

`invocation.json` now declares `metadata_version = 2` (older records without
this field are version 1). Existing fields and machine-result/manifest/registry
schema version 1 remain compatible; Core still reads historical metadata.
`toolchain` remains a string for compatibility and is always `"unknown"` in
version 2. Generic verification does not invoke `rustc` or infer a project's
toolchain from ambient PATH. Consumers that previously used this field as a
tool identity must use the selected step's runner description (program,
effective arguments and declared environment) and the collector's bound tool
provenance in its evidence instead. A runner description is a declaration, not
proof of a compiler version. `executor_version` identifies the Core package,
not the compiler used to build Core or the project's toolchain.

`commit` is optional checkout context sampled once during project discovery
and reused in reports, including staged invocations. It accepts only a valid
SHA-1 or SHA-256 commit ID from `git rev-parse --verify HEAD^{commit}`. The
probe has a one-second execution timeout, 4 KiB per output stream, an 8 KiB
combined budget and a 200 ms shared reader deadline, plus the executor's
bounded process-tree termination/reaping. Missing Git/HEAD, nonzero exit,
timeout, output overflow, invalid UTF-8 or malformed output produces
`commit = "unknown"`; working-tree source context likewise becomes
`working-tree:unknown`. Cancellation aborts the invocation with an error and
reclaims the probe tree rather than becoming optional success. No metadata
commands run while writing `invocation.json`.
Cancellation during discovery occurs before invocation allocation, so it emits
a CLI error without creating a successful invocation report.

This optional context does not satisfy required source/toolchain evidence.
Required Git operations (such as staged snapshot creation) and required
runner/collector gates retain their existing fail-closed semantics. Downstream
consumers must not treat `unknown` as a verified identity. Rollback to an older
Core restores the legacy metadata behavior, including its ambient Rust probe;
it does not rewrite retained reports, adopt a baseline or change a measurement
series.

Reports, logs, audit output, SBOMs, and webhook diagnostics redact cookies,
authorization headers, Bearer/Basic credentials, API keys, passwords, private
key blocks, and common database URL values. The latest 50 invocations are
retained; cleanup never removes an active or recently modified invocation.

Text evidence also recognizes JSON credential strings embedded in logs or JSON
lines, including escaped quotes, backslashes and Unicode escapes. Private-key
BEGIN blocks without an END marker are redacted through the end of the file.
Header values (including cookie attributes) are redacted after indentation or
explicit log prefixes: whitespace-separated bracketed fields, ISO date/time
stamps, TRACE/DEBUG/INFO/WARN/WARNING/ERROR/FATAL levels (with an optional colon),
and curl's `<`/`>` markers. Prefixes are preserved; arbitrary prose containing
`cookie:` does not establish a header boundary. These changes preserve the
existing evidence limits, generated-JSON identity checks and required quality
gate semantics described in the Engineering Policy.

Webhooks run after report writing:

    [[notifications.webhooks]]
    url = "https://hooks.example.test/events"
    allowed_hosts = ["hooks.example.test"]
    on_failure = true
    on_success = false

URLs must use http or https, have no userinfo or wildcard host, and list an
exact host in allowed_hosts. Each connection re-resolves the host and rejects
loopback, private, link-local, unspecified, multicast, shared (CGNAT
100.64.0.0/10), benchmarking, documentation and reserved addresses, including
IPv6 forms that embed a local IPv4 address (IPv4-mapped/compatible, NAT64
`64:ff9b::/96`, 6to4). Redirects
and proxy environment variables are disabled. Non-2xx responses, connection
errors, and policy denials fail verification with E1404 without changing the
written report. At least one result type must be enabled; webhooks run in
declaration order and stop after the first failure.

## 16. Signed out-of-process adapters

Adapters are not a step kind in flow.toml. The explicit harness-gate adapter
run command accepts a signed JSON request, a trusted Ed25519 key, and capability
allowlists. The contract is
[adapter-request.schema.json](../schema/adapter-request.schema.json), and the
protocol is described by
[ADR-0033](adr/0033-signed-out-of-process-adapter-protocol.md).

The host verifies protocol and result versions, executable SHA-256, complete
request signature, nonce/expiry replay protection, and artifact confinement
before spawning. It clears inherited environment and injects only declared
values. This is a protocol-level boundary, not an operating-system sandbox.
Bounded stdout/stderr and artifact limits map timeout, cancellation, crash,
non-zero exit, malformed output, signature, and confinement errors to the stable
ADAPTER_PROTOCOL_FAILURE code.

## 17. v1 to v2 checklist

The legacy v1 database recipe and old v2 external policies require the installed
GateProbes plugin. Core's `config migrate` rejects them with actionable help.
Generate a new file, retain the source, review the bound artifact, and validate
with a compatible Core before adoption:

    gateprobes-postgres migrate --input /path/to/project/legacy.flow.toml \
      --output /path/to/project/.harness-gate/flow.toml --project my-project
    harness-gate --project-root /path/to/project config check

Keep version = 2 explicit. When preflight rejects a file, follow the
diagnostic's primary field, related fields, and help text. Add explicit
dependencies, use distinct service injection names, and assign one log file per
step. Harness-Gate never silently reorders steps, inserts dependencies, or
rewrites configuration.

For generated schema files, validation commands, and release compatibility
guarantees, see the [schema catalog](../schema/README.md) and the repository
[README](../README.md).

Migration exit condition: all active configurations use explicit required checks
and a verified compatible released Core/plugin pair. Rollback restores the retained
flow and its compatible immutable Core/plugin artifacts together; do not combine
a migrated flow with v0.4.9 or downgrade checks to bypass validation. Engineering
Policy requiredness, thresholds, baseline/debt/ratchet and release authority remain
unchanged.

## Read-only verification plans

Use [verify --plan](verification-plan.md) to inspect the effective scope/profile selection and requirements without executing verification. The additive verification-plan/v1 output does not change configuration or policy versions.
