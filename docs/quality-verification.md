# Quality verification and unified reporting

`harness-gate verify` composes the configured execution workflow and the released
Rust generic evaluator into one blocking decision. Every required execution,
security, audit, evidence-publication and generic quality gate must succeed.
Collector exit status never grants policy approval. Repositories without
`.harness-gate/quality.toml` retain their existing verification behavior.

## Trusted workflow inputs

For each participating profile, add repository-relative host input paths to
`quality.toml`, alongside its existing collector and policy selections:

```toml
[profiles.full.workflow]
state = ".harness-gate/workflow-state.json"
trusted_keys = ".harness-gate/workflow-keys.json"
baseline_request = ".harness-gate/base-request.json"
```

`state` is the existing [trusted compiler state](quality-compilation.md).
`trusted_keys` is a JSON array of `{ "key_id": "...", "public_key": "..." }`
objects from the trusted adapter host. The optional `baseline_request` is the
existing [authenticated baseline request](quality-baselines.md). The host must
prepare these inputs from trusted configuration, pack metadata and authenticated
producer expectations. Collector output must never supply its own trusted state,
key allowlist or baseline expectations. Paths are confined to the execution root.

Verification loads these inputs before running execution gates. It checks the
state profile and resolves changed-subject aliases using configured component
ownership and resolved source paths: all/component scope includes matching
components; changed-file scope includes matching subjects. The signed state's
`changed_subject` set must match exactly. A mismatch requires freshly signed host
inputs; verification cannot rewrite authenticated collector requests. Critical
selection, mappings and exceptions remain host/compiler validated contracts.

After execution, verification resolves the configured baseline into an isolated
host directory, runs the selected profile's collectors through the signed adapter
host, validates their evidence and recompiles the pinned head inputs. Baseline
resolution precedes collection. The head artifact root must be fresh or match
the authenticated retained artifact inventory. Configuration, source or evidence mutation cannot become valid by passing
traditional steps. All collectors declared for the selected profile participate;
there is no implicit ecosystem-specific filtering or request generation.

A configured profile without trusted workflow inputs fails closed.
[Profile assurance](quality-profiles.md) defines omissions and retained reuse;
[reference presets](quality-presets.md) generate configuration but require host-provisioned runtime inputs. The explicit `step <id>` interface continues to run its selected
execution step without invoking project quality.

## Machine contract

The existing `test_result.json` schema version `1` gains an optional `quality`
object with discriminator `quality-verification/v1`. Its fields are:

| Field | Meaning |
| --- | --- |
| `status`, `phase`, `error` | `pass`, `fail`, `blocked` or `not_collected`; last orchestration phase; redacted failure detail |
| `participation`, `full_quality_status` | Selected and omitted policy expectations; complete assurance result (partial profiles always `not_collected`) |
| `producers` | Each selected producer was `collected` or `retained` |
| `selection` | Authenticated configured selection aliases |
| `inputs` | Compiled project/policy, canonical selection, bindings, context and input identity |
| `evidence` | Validated collector records, capability states and artifact provenance |
| `baseline` | Provider resolution and availability; retained compiled inputs/evidence when available |
| `project_report`, `project_report_path` | Complete authoritative `harness-project-report/v1` value and invocation artifact |
| `evaluation_time` | Exact exception-review time, for deterministic direct replay |
| `output`, `formats` | Configured report publication intent |

Unavailable stage outputs are `null`. Execution remains in `steps`, services and
failures; `QUALITY_BLOCKED` records generic failure independently. Top-level
`status`, `passed` and `evidence_complete` remain the final combined authority.
The embedded project report preserves the core's component aggregates, local and
cross-component gate records, relationship contracts, capability/evidence links,
baseline/ratchet/debt ledger and exception review without reinterpreting them.
Its existing `mode` field is preserved verbatim; workflow composition uses its
aggregate decision. No language identifier chooses semantics or report shape.

When policy participates, the authoritative report is a required invocation artifact. JSON and human copies
selected by `reporting.formats` are written under
`<reporting.output>/<invocation_id>/`, with the same invocation and combined
status. Standard invocation reports remain available on a quality failure.
Publication errors fail verification. Console and Markdown diagnostics render
subject/path, metric, base/head values, threshold, ratchet policy and outcome,
evidence/artifact links and configured remediation. This includes CRAP without
special language dispatch or a second threshold calculation.

## Direct evaluation and retention

`quality evaluate` remains the advanced interface. To replay a completed workflow,
write `inputs.project`, `inputs.policy`, `inputs.expected` and `evidence` to JSON
files, then use the existing `--project`, `--policy`, `--expected`, `--evidence`,
`--source-root` and `--artifact-root` arguments. Supply non-null compiled
`selection`, `mappings` and `exceptions` through their matching flags, and pass
`evaluation_time` as `--now`. For available baselines also supply `baseline.inputs`
project/expected/roots and `baseline.evidence` through all five `--base-*` flags.
The decision and complete project report match verification exactly.

Available baseline snapshots are retained at `baseline.retained_directory` outside
the working tree so direct replay can validate the original source and artifact
bytes. The host must retain these directories with the reports and remove them
when their replay retention expires. They are not included automatically when
copying only the report directory. Head artifacts likewise require host retention;
subsequent runs need fresh artifact roots and signed requests.

The end-to-end fixture uses `nebula-unregistered-2049`, an arbitrary configured
collector/tool/runtime and the supported generic `bundle.size` contract, including
a retained baseline and direct-evaluator equivalence. An architecture regression
guard rejects closed ecosystem dispatch in generic verification and reporting.


## Staged partial profiles

For a staged profile declared `assurance = "partial"` with no selected collectors
or policies, workflow state and trusted keys are read from the host checkout.
They do not need to be added to the Git index. The Core still validates every
configuration/source pin and the selected subjects against the immutable staged
snapshot, so a working-tree edit cannot replace staged source evidence.

Git cannot preserve empty directories. For this uncollected profile only, the
Core creates the configured artifact directories inside its private snapshot;
it does not import working-tree artifacts. State containing artifacts or retained
responses is rejected. Missing host state/keys, stale pins and unsafe paths stay
blocked, and successful hook execution reports full quality as `not_collected`.
Profiles selecting collectors or policies retain their existing authenticated
request and baseline requirements. This transport fix changes no engineering
policy, threshold, requiredness, measurement series, debt or release authority.

## Large native verification reports

Core-generated quality and machine-result JSON can exceed the 16 MiB limit for untrusted text files. The writer redacts the structured model before serialization, preserving JSON structure, numerical evidence and complete arrays. Each string/key remains bounded to 16 MiB, nesting to 128 levels, and the invocation evidence byte budget remains 256 MiB. Credential-bearing values are replaced without producing malformed JSON.

Only the exact SHA-256 of JSON produced and redacted by the current writer may pass the file-size boundary. Existing files, matching filenames or JSON extensions grant no exemption; modified generated output fails closed. External logs and evidence retain their existing 16 MiB limit. No quality policy, required capability, subject/series identity or baseline rule changes.

Structured serialization orders JSON object members deterministically. The reviewed CLI snapshot changes only member ordering in five machine reports; parsed fields, values, arrays and exit statuses are unchanged.
