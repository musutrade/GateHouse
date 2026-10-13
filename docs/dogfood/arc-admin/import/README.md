# Arc-Admin execution configuration import (GH-202)

GH-203 adds the separate [generic quality configuration](../quality/README.md) beside this unchanged execution import. Runtime blockers below still apply.

OpenSpec tasks 3.1–3.4 add an offline, deterministic import of the execution
declarations frozen by [GH-201](../README.md). This is not runtime shadow parity
or authority transfer. Existing Arc-Admin gates remain required.

## Use and reproduce

From a project root, run:

```bash
harness-gate config import --execution-only
```

The defaults read `.arc-flow/flow.toml` and write `.harness-gate/flow.toml` plus
`.harness-gate/flow.import.json`. `--input` and `--output` select paths inside the
project; `--project-root` selects that root. Import needs neither an existing
Harness-Gate config nor runnable project dependencies. It never executes commands,
reads environment values into configuration, generates policy defaults, or
overwrites existing output files. Choose a fresh output path for another import.

Without `--execution-only`, migration fails and lists runtime incompatibilities
before writing files. The explicit flag permits review of compatible execution
declarations; the report still records `authority_transfer: blocked`. Unsupported
or lossy declarations always fail, including with this flag.

Reproduce the committed fixtures from this repository into an unused path:

```bash
cargo run --manifest-path tools/harness-gate/Cargo.toml --locked -- config import \
  --execution-only \
  --input docs/dogfood/arc-admin/sources/.arc-flow/flow.toml.txt \
  --output target/quality/arc-admin-import/flow.toml
cmp target/quality/arc-admin-import/flow.toml docs/dogfood/arc-admin/import/flow.toml
cmp target/quality/arc-admin-import/flow.import.json docs/dogfood/arc-admin/import/flow.import.json
python3 -m unittest discover -s tools/quality/tests -p test_arc_admin_import.py -v
```

The JSON includes source/output SHA-256 values, every selected-step blocker,
required-list membership, dependencies, transformations and UX measurements.
Absolute paths, timestamps and host environment values do not enter the output.
Serialization is canonical for parsed TOML values; comments/formatting are not
retained. The report's source hash intentionally identifies the exact input bytes.

## Mapping and loss detection

| Baseline declaration | Imported contract and evidence |
| --- | --- |
| Project and aliases | Project identity, default/hook profiles, paths and all alias environment names are unchanged. |
| Components, scope, profiles | All seven scope rules, ordered patterns and component/profile sets are preserved; full selects 25 steps, hook selects 17. No `ci` profile is invented. |
| Commands and blockers | All 25 command definitions retain their IDs, order, arguments, working directories, logs and selection. All 23 explicit required IDs remain. `workflow.framework-release-config` and `workflow.framework-upgrade-tests` also remain blocking when selected despite their absence from the policy list. |
| Dependencies | Compatible `depends_on` declarations retain their order and references; missing/cyclic references fail validation. The baseline has no explicit dependencies and retains sequential declaration order. |
| Services and parsers | PostgreSQL image, isolation policy, environment injection, connection template, healthcheck and startup timeout remain intact. Regex patterns, captures and minimums remain intact. |
| Environment and timeouts | Alias/service/timeout override names and step `remove_env` declarations remain literal. Import does not resolve them against the host. Step and service timeout values are unchanged. |
| Arc defaults | Missing secret-policy path becomes `.arc-flow/secrets.toml`, rather than the Harness-Gate default. Each step explicitly sets `input = "repository"` to retain Arc-Flow's command access to the working tree. |
| Project validations | Playwright E2E, API/full-stack smoke, generation and deployment checks remain project commands, with no framework-specific Core changes. |

The importer accepts compatible Arc-Flow v2 declarations, plus compatible explicit
step dependencies. It rejects unknown versions, unsupported root/policy/step,
service or parser fields, invalid references, missing Arc-required step fields,
and values that would be dropped/coerced by Harness-Gate deserialization. A
recursive source projection check catches even fields ignored by flattened
decoders. Duplicate entries collapsed by set-valued fields fail rather than
silently changing the declaration. `${` is conservatively rejected anywhere in
source text, including comments, because Arc-Flow reads it literally whereas
Harness-Gate supports interpolation. Remove or rewrite such text explicitly.

## Remaining runtime blockers

The generated file preserves declarations. Before replacement can be considered:

- Resolve global `ARC_FLOW_REPORTS`, `AUDITOR_CONFIG`, `ARC_FLOW_AUDIT_CONFIG` and
  `ARC_FLOW_SECRETS_CONFIG` usage. These implicit Arc-Flow overrides are not
  imported. Harness-Gate uses its own environment interface, protects audit
  policy overrides, and treats `REPORT_DIR` as a deprecated compatibility alias.
- Retain and validate the actual audit/secret policies and their mandatory
  blocking behavior. Import preserves policy paths but does not copy or certify
  those files. Hook staged input and working-tree prelude behavior, PostgreSQL
  lifecycle and command outcomes still need matched shadow evidence.
- Route `.harness-gate/` through project scope and integrate hook/CI invocations
  deliberately. The unchanged Arc scope rules do not add destination routing.
  Generic quality configuration and any `ci` profile are separate work.

These are recorded in every import report and printed to the operator. They are
not waivers or successful runtime checks. OpenSpec tasks 4–9 remain outstanding.

## Measured product UX

| Metric for the frozen baseline | Result |
| --- | ---: |
| Import commands | 1 |
| Manually re-entered steps / manual execution-config edits | 0 / 0 |
| Preserved source scalar values | 491 |
| Original configuration bytes | 12,229 |
| Generated configuration bytes | 13,505 |
| Retained original configs / additional generated configs | 1 / 1 |
| Duplicated step definitions during coexistence | 25 |
| Operator elapsed time / remaining runtime integration effort | Unmeasured |

These are counts of the reproducible import operation, not a claim that complete
project adoption takes zero effort. Configuration coexistence adds 13,505 bytes
and duplicates all 25 step definitions; the JSON report is additional evidence,
not execution configuration. Manual runtime integration effort and human elapsed
time remain unmeasured and are not encoded as zero. There is no duplicate-command
execution measurement or CI cost-saving claim in this task.

The Rust CLI fixtures check deterministic bytes under different host overrides,
dependencies/defaults, blocker/reference rejection and refusal to overwrite or
escape the project. Independent Python tests compare every parsed source value
against GH-201's frozen inventory and verify hashes/counts. See
[validation evidence](validation.json) for actual local results and limitations.
