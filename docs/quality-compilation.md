# Trusted quality compilation v1

Project-level signed execution is described in [Trusted project collectors](quality-collectors.md).

GH-180 implements OpenSpec tasks 2.1–2.3. The compiler translates validated
quality configuration and host-owned state into the existing generic Rust
contracts. It resolves references; the Rust generic core evaluates policy.

```sh
harness-gate quality compile --repository-root . --state trusted-state.json --output compiled.json
harness-gate quality evaluate --repository-root . --state trusted-state.json --evidence evidence.json --output report.json
```

The second command recompiles and validates the inputs on every invocation, then
uses the same evaluator and report builder as direct `quality evaluate`. The
first writes an inspectable `quality-compiled-inputs/v1` bundle containing
`project`, `policy`, `expected`, `source_root`, `artifact_root`, `selection`,
`mappings`, `exceptions`, and `identity`. Its embedded contracts can also be
supplied to the direct command. A serialized bundle is not an authenticated
cache or a release decision.

## Trusted inputs

Compilation uses the default `.harness-gate/flow.toml` and `quality.toml` under
`--repository-root`. The strict `quality-trusted-state/v1` JSON contract is
illustrated by the retained [fixture](../tools/harness-gate/fixtures/workflow/compiler/state.json).
Its fields are:

- `profile` and `expected`: selected quality profile and the generic core's exact
  commit/base-commit/target/run context, resolved by the caller.
- `config_files`: exact repository-relative inventory of flow, quality, every
  referenced policy file and the selected collectors' request files, with SHA-256
  digests of their bytes. Keys must match configured paths exactly; use `/`
  separators when producing this inventory on Windows as well as Unix.
  Configuration must pass existing cross-plane validation.
- `components`: configuration aliases mapped to generic component metadata from
  trusted packs. IDs and source boundaries must agree with configuration.
- `subject_kinds` and `relationship_kinds`: used configuration kind aliases mapped
  to generic contract kinds by trusted pack data, with an exact alias inventory.
- `subjects`: each configured subject alias mapped to a nonempty set of resolved
  generic subjects. Explicit paths must agree exactly; discovery results are
  supplied by the trusted resolver. Ownership, kind, target, source bytes and
  canonical subject IDs are checked against configuration and the core.
- `series`: exactly the selected collectors, each mapped to its generic series
  metadata. The canonical core series digest, target, configured capabilities and
  configured series IDs must agree, including collector/tool/runtime versions.
- `artifact_root` and `artifacts`: a repository-contained root and immutable
  relative-path/SHA-256 manifest. Files must exist inside configured component
  artifact roots. Evaluation also checks each evidence artifact against its
  owning component and the manifest, and each capability against the selected
  subject/capability/series bindings.
- Optional `selection`: `changed_subject` and `critical_subject` alias sets,
  resolved to sorted canonical subject IDs. Optional `mappings` and `exceptions`
  carry the existing generic contracts unchanged to core validation/evaluation.

This boundary assumes state comes from the trusted host and pack resolver, never
from collector-controlled metadata. It checks consistency and bytes, not host
signatures or Git provenance. [Collector execution](quality-collectors.md) and
[baseline resolution](quality-baselines.md) expose separate trusted stages;
[verification composition](quality-verification.md) consumes these stages. Base evaluation
still accepts the existing complete set of five
trusted base arguments. Profiles without policy rules do not produce an evaluable
policy; empty-profile lifecycle behavior remains a later profile-semantics task.

## Determinism and identity

The compiler emits `harness-project/v1` and `harness-policy/v1`. Components,
relationships and rules are sorted by identifier; subjects and selection members
are sorted by canonical subject ID. A subject-scoped policy reference expands to
one rule per resolved subject, named `<rule-id>@<subject-id>`. Duplicate generated
rule IDs fail. Component and relationship scopes retain their generic rule data.
No threshold, CRAP, debt or ratchet calculation occurs during compilation.

Identity is `quality-compilation/v1:<sha256>`, over the core's canonical JSON of
`{version, compiler, config, flow, state, project, policy, selection}`. `compiler`
is the crate version; `config` and `flow` are validated resolved models; `state`
uses sorted map/set keys and resolved subject arrays sorted by ID. Source,
config, tool/series, profile/scope, context, artifact manifests and optional
contracts therefore contribute to identity. Other arrays retain their declared
order, including the core series metric order. Configuration pins deliberately
bind raw bytes, so even a configuration formatting change requires fresh pins.
Absolute output mount roots are excluded; relative configured roots remain
bound. Changing compilation semantics requires a compilation version change.

Missing, mixed or stale inputs fail closed. Existing destination artifacts are
removed before processing errors can leave a prior report or bundle in place;
outputs that alias known inputs are rejected. Adding an ecosystem/language name
does not require a compiler enum or branch. Ecosystem metadata and custom
collector/capability/series identifiers follow the same generic translation;
the core remains responsible for rejecting unsupported metrics at evaluation.

## Retained equivalence evidence

`tools/harness-gate/fixtures/workflow/compiler/acceptance.py` drives the actual CLI
from the nextest integration suite. It retains separate direct and compiled
reports and compares their JSON and exit status, checks expected PASS/FAIL,
checks repeated compilation and reordered subject discovery, and exercises
stale/mixed input rejection with stale output cleanup. The unknown ecosystem
fixture uses `nebula-unregistered-2049`, a custom collector and series, and
`quasar.pulses`; compilation succeeds and evaluation fails closed because the
core does not support that metric. A variant with supported coverage succeeds
without changing the ecosystem or compiler path.
