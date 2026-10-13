# Quality participation and retained evidence

Profiles select configured collectors and policy bindings. `assurance` defaults
to `complete`: every configured required policy must participate, with exactly
one compatible producer for its target/capability/series. `partial` permits
omissions. Neither profile names nor ecosystem names determine participation.
Packs describe expensive and cheap work through these selections; the host does
not maintain a language or cost-class switch.

```toml
[profiles.hook]
assurance = "partial"
collectors = ["cheap"]
policies = ["cheap"]

[profiles.full]
collectors = ["cheap", "expensive"]
policies = ["cheap", "expensive"]

[profiles.ci]
collectors = ["cheap", "expensive"]
policies = ["cheap", "expensive"]
```

The identifiers above refer to pack-supplied collectors and policy bindings, not
built-in capabilities. Any custom profile can use either assurance level. A
complete profile that removes a required binding or producer fails configuration
validation. Existing flow-only repositories and `generic` retain opt-in behavior.
[Reference presets](quality-presets.md) now compose coherent quality configuration
from reusable ecosystem and capability packs (OpenSpec tasks 7.1–7.5).

The certified Rust reference fixture binds required line/region coverage and
CRAP in both complete profiles. Its retained-reference regression compares exact
policy limits, requiredness and the entire original measurement series: coverage
4/5 and CRAP <= 30 remain unchanged. Native boundary-to-component mapping belongs
to the pack. Angular/TypeScript CRAP remains `unsupported`, with no numeric value
and no invented series. Requiring that unsupported capability fails evaluation.
The `nebula-unregistered-2049` fixture selects cheap and expensive capabilities
for hook/full/ci and a custom profile using the identical generic configuration.

## Truthful reports

`quality-verification/v1` includes `participation` with profile, assurance and
all configured policy expectations. Unselected expectations are `not_collected`
with reason `omitted by profile`; they are not synthesized evidence records.
`full_quality_status` is always `not_collected` for a partial profile, including
when every selected cheap check passes. Selected policies still fail normally.

A profile without selected policy has status `not_collected`, no project-policy
report and no evaluator invocation. A profile without collectors has an empty
evidence list and launches nothing. Baseline evaluation is unnecessary when no
policy is selected. Execution success can satisfy that partial workflow without
asserting full-quality PASS. Complete profiles report their actual evaluation
result. Configuration or collection errors block the workflow.

## Retained head collection

The authenticated host can supply `retained` in the trusted compiler state,
keyed by selected producer ID:

```json
{"retained":{"expensive":{"path":".harness-gate/retained-expensive.json","sha256":"<host-pinned SHA-256>"}}}
```

Each file is a `quality-retained-response/v1` envelope containing `binding_digest`
and the original adapter `response`. `quality collect` publishes these envelopes
in `responses`, alongside validated evidence and producer origins (`collected`
or `retained`). That output alone confers no trust: the host must authenticate
its producing run and pin both response bytes and raw artifact inventory in
trusted state. Collector output cannot nominate its own trusted pins.

Reuse requires the current compiled binding (configuration, project, context,
selection, profile, policy, mappings, exceptions and series), signed request
identity/claims, source hashes, response provenance and artifact hashes to match.
Different invocations or profiles need their own authenticated producer inputs;
this is not a cross-run or full-to-ci cache. All retained responses are checked
before any fresh collector launches. Missing, stale, altered or incompatible
retained evidence blocks the invocation without a recollection fallback.

An accepted retained producer skips adapter execution entirely, including its
already-consumed launch nonce. The trusted host's authenticated retention replaces
fresh launch authentication; existing signed request bytes remain pinned and
bound. Other selected producers collect once. Reused artifacts cannot change
during fresh collection, and the final evidence must account for every artifact.
The same released evaluator owns the resulting decision. No new metric authority,
threshold, measurement series or ecosystem certification is introduced.

Under the [accepted CI model](quality/ci-topology/README.md), Quality Coverage and
Critical Paths remains the collection owner and Required Quality Aggregate stays
lightweight. This change adds no hosted job or duplicate measurement. Actual
workflow wiring and hosted timing capture remain OpenSpec section 8; the local
implementation and cost boundaries are recorded in
[GH-184 validation](quality/gh-184/validation.md) and
[ADR-0046](adr/0046-capability-driven-quality-profiles.md).
