# Configure the native CRAP gate

The standalone native plugin's `evaluate` command requires a project-owned CRAP
policy. Core validates that policy and makes the gate decision. There is no
implicit threshold or compatibility fallback when configuration is absent.

## File and field

In `.harness-gate/quality.toml`, select the existing policy binding, for example
`policies."app.risk.crap"`. Its `policy_file` identifies a repository-relative
`harness-policy/v1` JSON file; `rule` selects exactly one entry in `rules`.
Change that entry's `limit.numerator` and `limit.denominator`:

```json
{
  "id": "app.risk.crap",
  "scope": { "kind": "component", "component": "app" },
  "metric": "risk.crap",
  "operator": "le",
  "limit": { "type": "rational", "numerator": 30, "denominator": 1 },
  "required": true,
  "on_violation": "fail",
  "ratchet": { "deny_regression": true, "allow_legacy_debt": true },
  "remediation_classes": ["review_native_production_evidence"]
}
```

This is one rule inside `{"schema":"harness-policy/v1","rules":[...]}`.
`30/1` means an inclusive ceiling of 30; `111/2` means 55.5. Core compares exact
rationals. A larger configured ceiling is not clamped to 30. The Rust reference
preset and this repository's required CI retain their existing default of 30.
Do not put the threshold in `[collectors]`, an environment variable or plugin
options. See the complete [quality configuration contract](../quality-configuration.md).

The selected binding must describe `risk.crap` on one component and its exact
native function measurement series. Its producer must declare the same series.
A reference/stable series, another target or another native projection is not
interchangeable. The component's `source_roots` must contain the capture's complete
source inventory; use the package root for a capture that includes the whole package.

## Run the configured gate

Supply the project root and the binding name explicitly:

```sh
harness-gate-rust-collector evaluate \
  --sysroot /absolute/external/rust-1.97.1 \
  --harness-gate /absolute/bin/harness-gate \
  --repository-root /absolute/project \
  --policy-binding app.risk.crap \
  --project my-project \
  --base /absolute/evidence/base/raw --base-anchor BASE_HOST_RETAINED_SHA \
  --head /absolute/evidence/head/raw --head-anchor HEAD_HOST_RETAINED_SHA \
  --base-context /absolute/host/base-context.json \
  --head-context /absolute/host/head-context.json \
  --hotspots /absolute/host/hotspots.json \
  --output /absolute/evidence/new-evaluation
```

`--project` is the quality project **ID**, matching `[project].id`; it is not a
filesystem path. Contexts and capture anchors remain trusted host inputs, and
the head context must reference the base commit. Hotspots are a JSON array of
compiler names, possibly `[]`. Reviewed explicit lineage can be supplied through
`--mappings`. The output directory must be new.

`evaluate` first calls `harness-gate config check` on the project. It retains
`config-check.*`, exact configuration bytes in `policy-inputs/`, and a
`policy-binding.json` receipt naming the binding, source rule, ceiling, component
and SHA-256 of each consumed policy input. `policy.json` contains the rules sent
to Core; `report.json` contains Core's decisions and effective per-function limits.

An exact-series mismatch blocks evaluation and reports the configured and
measured identities. The certified projections remain in `base-evidence.json`
and `head-evidence.json` for host review. Use the function record's `series.id`
when establishing the native binding; do not copy the aggregate coverage series
or automatically accept an unfamiliar identity. This projection pins its actual
adapter source and tool/selection inputs. Reproject both immutable raw captures
together when adopting it, retaining the original anchors and normalized reports.
Do not compare the new projection directly with another normalized series or
reset history. Changing only the policy limit does not change measurement facts
or their series identity.

## Scope and failure behavior

This convenience command evaluates the explicitly selected native capture and
CRAP binding. It does not certify every policy/component in a quality profile.
Use Core's [configured verification flow](../quality-verification.md) for that.

The native bridge requires a rational inclusive CRAP ceiling, `required=true`,
`on_violation=fail`, and an explicit `ratchet` with `deny_regression=true`.
Core consequently requires a configured required baseline provider.
`allow_legacy_debt=false` makes every CRAP rule absolute. With an explicit `true`,
only unchanged, unselected functions may retain historical
debt. New/changed functions, hotspots and changed functions with CC > 10 retain
their absolute checks. Coverage rules remain at 80%; this change wires the CRAP
ceiling, without adding a separate coverage configuration interface.

Missing files/rules, duplicate JSON keys or rule IDs, invalid rationals, unsupported
operators or weakened requiredness/non-regression settings, escaping paths,
component/series mismatches and configuration mutation during validation all
block. None falls back to a fixed ceiling or becomes a macro exemption.
Raising the ceiling does not permit a measured base/head regression. Damaged or
ambiguous measurement evidence and missing/incompatible baselines still block.

The installed API requires the two new configuration arguments. Historical
invocations without project configuration are not a supported execution branch.
