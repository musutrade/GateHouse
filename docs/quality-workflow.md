# Project quality workflow

Core and the optional Rust plugin have separate installation and certification
boundaries. Start with the [current releases](release-status.md) and
[Rust installation guide](quality/rust-collector-installation.md).

**`init -> verify -> one project decision`** is the normal product workflow.
Configuration selects execution, collectors, capabilities, policy and profiles.
Collectors measure; the released Rust evaluator decides quality. `verify`
combines that decision with required security, audit and execution outcomes.
`quality evaluate` and `adapter run` remain advanced replay/debug/integration
interfaces; they do not replace the complete project verification command.

## Quick start

Run in an existing Git project with the project's build tools available:

```bash
harness-gate init --preset rust-api --presets-dir /path/to/GateProbes/presets
harness-gate config check
harness-gate doctor
# Review generated QUALITY.md and provision trusted host inputs described below.
harness-gate verify --profile ci --all
```

Choose `rust-api`, `angular-only` or `angular-rust-postgres` for the current
reference configuration. Review paths, commands and profiles in `flow.toml`,
quality bindings in `quality.toml`, and generated policy/capability packs.
`init` does not install collectors, create keys, sign requests or prepare runtime
state. The host must provision each selected profile's fresh signed collector
requests, trusted compiler state and key allowlist (generated presets reference
`.harness-gate/runtime/`). See [trusted compilation](quality-compilation.md),
[collector inputs](quality-collectors.md) and [verification inputs](quality-verification.md).
Even `hook` requires its configured trusted state. Missing inputs block verification.
Configuration validation alone is not evidence of measurement or quality PASS.
For read-only snapshots, select persistent host-controlled nonce storage with
`--replay-state-dir PATH`; `verify` and `quality collect` must use the same ledger.
See [replay scope and lifecycle](quality-collectors.md#durable-replay-protection).

Inspect the final `test_result.json` status and linked project report. A successful
command step cannot override required quality failure, and quality success cannot
override a security, audit, execution or report-publication failure. A partial
profile can pass its selected work while `full_quality_status` stays
`not_collected`. Before delivery select a complete profile such as configured
`ci`; the profile name alone does not establish complete assurance.

The `generic` preset creates execution configuration only. Existing flow-only
repositories keep their current behavior. This tool's own source checkout has no
project-local `flow.toml`; root config check and CI verify are not applicable here.
Its repository checks and configured temporary fixtures provide validation instead.

## Architecture and extension

```text
init: reference recipe + reusable packs -> flow.toml + quality.toml + policies
verify: configured profile/scope + host-authenticated state
  -> security/audit/execution gates
  -> baseline resolution + signed collector orchestration / validated retention
  -> deterministic trusted project/policy/evidence compilation
  -> released Rust evaluator -> project quality report
  -> combined required gates -> one blocking project decision
CI: one measurement owner per series -> validated artifacts -> required aggregate
```

Ecosystem, language and framework identity is configuration/pack data. The generic
schema, compiler, orchestrator, baseline, profile, verify, report and CI layers do
not require a core code change for a new language name. The extension path is
**collector + capability/policy pack + certification**, without generic-core
redesign. A collector supplies normalized facts and provenance under supported
metric contracts; packs supply identity, series, targets, capability states and
policy bindings. Certification must retain actual collector/series/toolchain
evidence, equivalence and failure cases. Arbitrary identity is accepted as data;
unsupported metric semantics still fail closed and require separate contract work.

The [unknown-ecosystem closure fixture](quality/gh-187/validation.md) demonstrates
composition, validation, compilation, signed collection, baselines, profile
selection, direct/verify report parity and the shared CI acceptance runner. It
does not certify a real Nebula language, collector or application.

Current reference presets are conveniences, not the long-term extension limit.
The composable-pack direction supports future selections such as Vue + Go +
PostgreSQL through reusable component packs and independent contract/capability
packs. PostgreSQL is currently an execution service; it contributes quality only
through an explicitly selected capability. Built-in catalog and flow templates
remain reference data. Registry discovery, installation, version resolution and
execution-pack composition are future work, not current CLI features. See
[pack composition](quality-presets.md) and [ADR-0047](adr/0047-composable-quality-preset-packs.md).

Application-specific validation (API, E2E, integration, smoke, migration, load,
generation or framework tooling) stays project-owned. Harness-Gate contributes
command hooks for orchestration, structured result adapters for diagnostics, and
measurement-only collector plugins; the released Rust core keeps authority over
requiredness, thresholds, baselines and final decisions. A new framework must
not force a generic-core change to run through a command hook. See
[ADR-0049](adr/0049-project-owned-validation-extension-boundary.md) and
[Engineering Policy section 7](engineering-policy.md).

## Configuration and authority

`flow.toml` v2 owns commands, services, paths, scope, execution profiles and
parsers. Optional `quality.toml` v1 owns project/component/subject relationships,
source/artifact boundaries, collector expectations, policy bindings, profile
participation, baseline strategy and reporting. The [quality schema reference](quality-configuration.md)
documents every table and cross-plane constraint; use `config print --quality --resolved` to inspect it and `schema export --quality` to export its schema.

Policy files own requiredness, thresholds, debt, ratchets and exceptions. Collector
declarations cannot grant requiredness, threshold overrides, aggregate PASS/FAIL
or release authority. Host-owned keys and expected identities cannot come from
collector output. The adapter capability allowlist is a protocol-level check,
not an operating-system sandbox. See [collector trust boundaries](quality-collectors.md).

## Capabilities and certification

| State | Interpretation |
| --- | --- |
| `supported` | The collector declares support; only validated applicable evidence supplies a value. This is not policy PASS. |
| `unsupported` | The collector/series cannot measure this capability. |
| `not_configured` | The capability has not been bound for this setup. |
| `not_collected` | No measurement for this invocation/profile, including declared partial omissions. |
| `not_applicable` | The capability does not apply to the subject, with contract-valid justification. |
| `measurement_error` | Measurement failed; never reinterpret it as unsupported or success. |

Unavailable states are not zero, 100% coverage or favorable synthetic metrics.
Required applicable evidence fails closed when missing or invalid. Optional
unavailable capabilities remain visible without claiming certification.

Rust reference full/CI policy uses the accepted coverage/risk series, including
production-function **CRAP <= 30** and unchanged debt/no-regression rules. Its
[risk evidence and support boundary](quality/function-risk.md) and
[Engineering Policy](engineering-policy.md) govern applicability. TypeScript/Angular
certification covers bounded original-source line/function coverage and retained
quote-contract/generated-client checks for the exact fixture, tools and series
in the [certification matrix](quality/typescript-certification.md). Angular CRAP,
complexity, branch/region coverage, browser/SSR and other unmeasured capabilities
remain unavailable. Never combine unrelated coverage and complexity into CRAP.
A preset, successful build or synthetic receipt cannot certify an ecosystem.

## Baselines, profiles and artifact trust

[Baseline providers](quality-baselines.md) support `none`, Git ref/merge-base and
retained artifact manifests. A Git provider materializes a pinned source and
validates the host-supplied baseline evidence; retained providers validate
authenticated bundle identity, hashes and compatibility. Scheduling baseline
collection and authenticating CI remain host responsibilities. New reference
presets default to `none`, without a debt
waiver. `deny_regression` or `allow_legacy_debt` requires a required provider and
trusted baseline request. Missing required, corrupt or incompatible baselines
block; optional absence is explicitly unavailable. Equal metric names or numbers
do not establish measurement-series compatibility.

[Profiles](quality-profiles.md) select bindings using capability/series metadata,
not language names. Complete profiles include every configured required policy;
partial profiles honestly record omissions. Hook may omit expensive coverage/CRAP
while full/CI retain responsibility. Custom profile names use the same rules.

The host pins source, configuration, subject selection, producer, tool, series,
run/invocation and artifact inventory. Retained responses and artifacts must
match those authenticated expectations and digests before reuse. Mutation, stale
identity or a missing envelope blocks without fallback or duplicate collection.
In CI, `quality-coverage` remains the production coverage/risk/CRAP owner; artifact
sealing and independent manifest validation precede consumption. The lightweight
Required Quality Aggregate checks required child outcomes; it never recollects or
evaluates generic policy. Reference fixture artifacts are not evidence for another
application. See [CI ownership and trust](quality/ci-topology/README.md).

## Migration and rollback

Generate a reference in a separate directory; review and copy quality/policy
files, align project identity, paths and flow profiles, then provision trusted
inputs and run config check and complete verification. Follow the exact
[adoption steps](quality-presets.md#enablement-and-migration).
`config migrate` only migrates execution schema v1 to v2. `init --force` replaces
generated files and is not an adoption shortcut. `generic` preserves existing quality.

Rollback is an explicit reviewed configuration change: retain reports, policies,
series and baselines; remove/disable the quality integration to restore flow-only
execution where authorized, or revert to the previous accepted integration.
Flow-only success is reduced assurance and cannot satisfy a required quality gate.
Keep required CI evidence evaluation through the released Rust evaluator while
repairing integration. Never fall back to Python or collector-owned approval,
reset debt, or promote stale/reference evidence. The [closure record](quality/gh-187/validation.md)
retains compatibility and fail-closed tests plus this rollback boundary.

## Troubleshooting

| Symptom | Action |
| --- | --- |
| `HGCFG-QUALITY` | Run `config check --format json`; fix the reported references, policy/producer series, paths or profile participation. |
| Missing workflow input after init | Read generated `QUALITY.md`; provision host state, signed requests and trusted keys for the selected profile. |
| Signature, nonce, source/config or selection mismatch | Regenerate authenticated host inputs for the actual invocation and scope; do not edit signed payloads or relax trust checks. |
| Retained artifact or manifest mismatch | Preserve the failure evidence, locate the correct authenticated owner artifact, or start a fresh authorized measurement invocation; do not silently recollect in reuse mode. |
| Baseline unavailable/incompatible | Restore the pinned baseline/ref and lineage or explicitly review a series migration; never use head as base. |
| Steps pass but verify fails | Inspect top-level failures, `quality.phase/error`, project blockers and evidence links in `test_result.json`; required quality/publication failures still block. |
| Hook passes but full quality is `not_collected` | Run the configured complete profile; omission is not full assurance. |
| Unsupported CRAP or custom metric | Check actual collector/series certification; add a certified pack/collector or propose separate metric contract work. Do not invent values. |

For direct reproduction use the advanced `quality evaluate` command with the
retained exact compiled inputs and evaluation time. See the
[unified report contract](quality-verification.md) and [failure codes](failure-codes.md).
Java/Python/third-ecosystem adapters, compat deprecation, risk-driven conditional
hosted platforms and real application dogfood remain outside this change;
follow-ups are deferred until acceptance.
