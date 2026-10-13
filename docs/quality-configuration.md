# Quality configuration v1

`.harness-gate/quality.toml` declares language-neutral quality intent alongside
`flow.toml` v2. The [generated schema](../schema/quality.schema.json) describes its
closed structural contract. `harness-gate config check` validates both files,
including their references and policy-owned requirements. Invalid quality input
produces `HGCFG-QUALITY` diagnostics in `config check --format json`.

This is the configuration layer delivered by GH-179, OpenSpec tasks 1.1–1.4.
GH-180 adds [trusted compilation](quality-compilation.md), tasks 2.1–2.3.
[Collector execution](quality-collectors.md) and [baseline resolution](quality-baselines.md)
are separate stages; [verification](quality-verification.md) composes their results.
A successful config check proves configuration consistency;
it does not certify evidence or establish a quality PASS.

## File presence and commands

A repository without this file retains existing flow-only behavior. Schema export
and the `generic` preset do not create it. New reference presets explicitly include
quality; see [preset defaults and migration](quality-presets.md). A present
malformed, unreadable or broken-symlink configuration fails config validation.
The file is located under the execution root even when `--config` selects an
alternate flow file.

```sh
harness-gate config check
harness-gate config check --format json
harness-gate config print --quality
harness-gate config print --quality --resolved
harness-gate schema export --quality
harness-gate schema export --quality --output schema/quality.schema.json
```

`config print` continues to print flow configuration by default, after validating
both planes. `--quality` prints the raw quality file; `--resolved` serializes the
validated model with defaults. It errors when quality is absent. Quality paths
are literal repository-relative paths without environment interpolation, drive
prefixes, backslashes, parent traversal or symlink escapes. Declared source,
request, artifact and report paths may be created later; referenced policy files
must already exist. Checking configuration never launches a collector or fetches
a baseline.

## Model

The complete [quality fixture](../tools/harness-gate/fixtures/workflow/quality.toml)
and its [policy document](../tools/harness-gate/fixtures/workflow/policy.json) pair with
the `rust-api` flow preset. Docs consistency checks this pair in a temporary
project and also cross-validates all generated reference presets and flow-only migration. Its illustrative series hash is not
certified measurement evidence.

| Table | Meaning and constraints |
| --- | --- |
| `version` | Integer `1`; unknown fields are rejected throughout the model. |
| `project` | Stable `id` and `name`; name must equal `flow.project.name`. |
| `components.<id>` | Nonempty `flow_components` subset of flow step components, nonempty `source_roots`, and `artifact_root`. |
| `subjects.<id>` | Owning `component`, language-neutral `kind`, and `selection`. Optional map, empty by default. |
| `relationships.<id>` | Kind plus distinct `from`/`to` component or subject targets. Relationship endpoints cannot themselves be relationships. Optional map, empty by default. |
| `collectors.<id>` | Protocol `harness-collector-request/v1`, repository-relative `request` material and nonempty `produces` expectations. |
| `policies.<id>` | `policy_file`, `rule` ID and `expectation`. The existing `harness-policy/v1` document owns requiredness, limits, violation behavior and ratchet settings. |
| `profiles.<name>` | `assurance` defaults to `complete`; explicit `partial` permits omissions without full-quality PASS. Explicit sets of `collectors` and `policies`; names must occur in flow step profiles. At least one profile must be declared; empty participation sets are permitted. |
| `baseline` | Explicit `required` flag and a tagged `provider`. |
| `reporting` | Repository-relative `output` directory and nonempty `formats` set containing `human`, `json`, or both. |

Map keys, project IDs and subject/relationship kinds use lowercase identifiers
starting with a letter and continuing with letters, digits, `.`, `_` or `-`.
Explicit selection uses `{ kind = "explicit", paths = ["src/lib.rs"] }`; each
path must remain inside its owning component's source roots. Discovery uses
`{ kind = "discovery", collector = "coverage", query = "modules" }`, with a
nonblank opaque query and an existing collector. Discovery queries are intent;
the later trusted compiler must validate discovered subjects against boundaries.

An expectation consists of `target = { kind = "component", id = "app" }`, a
capability such as `coverage.line`, and the complete
`measurement-series/v1:<64 lowercase hexadecimal digits>` identity. Target kinds
are `component`, `subject`, and `relationship`; every ID must resolve. Capability
names have two or more dot-separated lowercase segments, with digits and
underscores allowed after each segment's initial letter.

## Cross-plane and authority validation

Within each participating profile, a target/capability has at most one producer.
A bound policy and its producer must declare the exact same measurement series;
equal metric names or numeric values never establish compatibility. Complete profiles must select every configured required policy. A selected required
policy must have a producer in that profile. Optional policies may have no
producer, but an existing producer must still match. Discovery collectors for
active targets (including component subjects and relationship endpoints) must
participate in that profile. Undeclared collector, policy, component, subject,
relationship and profile references fail closed.

Policy bindings name an exact component, subject or relationship scope and metric
in the referenced policy rule. Policy documents use the frozen strict JSON parser
and policy schema, including duplicate-key/rule rejection and the shared metric,
limit type and relationship-policy checks. The config validator
reads participation requirements; the existing Rust evaluator remains responsible
for policy evaluation against compiled project and evidence inputs.

Collectors may only declare measurement expectations. Fields such as `required`,
`threshold`, `ratchet`, `pass`, `fail`, `aggregate` and `release_authority` are
rejected at the collector, expectation and target levels. Inline policy overrides
are rejected. Configuration cannot promote a collector into delivery authority.

Baseline providers are `{ kind = "none" }`,
`{ kind = "git", reference = "origin/main", merge_base = true }`, or
`{ kind = "retained_artifact", manifest = "target/baseline-manifest.json" }`.
A required baseline cannot use `none`. A policy enabling `deny_regression` or
`allow_legacy_debt` requires a required provider; relationship policies cannot use
debt ratchets. Config checking validates provider intent and path containment,
without asserting availability, source freshness or series compatibility of an
unloaded baseline. The [baseline provider](quality-baselines.md) performs those checks.

The [Engineering Policy](engineering-policy.md),
[optimized CI topology](quality/ci-topology/README.md), and
[ADR-0040](adr/0040-language-agnostic-evidence-policy.md) remain normative. CRAP,
coverage, debt, ratchet, fail-closed and measurement-series semantics are unchanged.
The current Rust path remains the sole release authority. Schema/docs validation
uses the existing CI jobs and adds no collection owner or aggregate evaluator.
See [ADR-0041](adr/0041-quality-configuration-v1.md) for this bounded decision.

See [profile assurance and retained evidence](quality-profiles.md) for hook/full/ci participation.

For direct standalone native evaluation, see [native CRAP policy configuration](quality/native-crap-policy.md).
The command selects `policies.<binding>.policy_file` and `.rule` explicitly; the
referenced rule's rational `limit` is the ceiling Core actually evaluates.

## Collection artifact capacity

`quality.toml` can set a host-reviewed artifact budget for both `verify` and
`quality collect`. Omission retains the 64 MiB default. The positive byte count
covers the entire shared artifact directory, including files not declared by a
collector; it is not a per-file or per-collector allowance. Zero and unlimited
values are rejected. Configuration changes invalidate signed collection inputs.

```toml
[limits]
max_artifact_bytes = 1073741824 # 1 GiB
```

This is a storage resource limit, independent of coverage and complexity policy.
