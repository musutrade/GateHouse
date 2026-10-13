# Arc-Admin generic quality configuration (GH-203)

This reviewable shadow configuration implements OpenSpec tasks 4.1–4.4 against the [frozen Arc-Admin baseline](../README.md) and [GH-202 execution import](../import/README.md). GH-204 subsequently staged it in a disposable pinned Arc-Admin snapshot: [shadow observation](../shadow/README.md) records production loading rejection before execution. Successful runtime parity, cost measurement and authority transfer remain pending.

To stage it in Arc-Admin, place `quality.toml` at `.harness-gate/quality.toml` and `packs/` at `.harness-gate/packs/`, beside the unchanged imported `flow.toml`. These files compose the existing `angular-rust-postgres` packs, with Arc-Admin identity, the imported profile vocabulary and a required baseline provider. They do not add a project-local flow to Harness-Gate's own repository.

| Binding | Quality participation |
| --- | --- |
| `frontend` (`frontend/src`) | Native TypeScript line/function coverage ≥4/5; CRAP explicitly `unsupported`, optional diagnostic with no numeric substitute |
| `backend` (`backend/src`) | Certified Rust line/region coverage ≥4/5 and CRAP ≤30 |
| `frontend-api` (`frontend` → `backend`) | Required API-contract breaking changes =0, generated-client drift =false, compatibility =true |

The relationship collector consumes normalized contract facts from the existing native contract capability. It does not turn the API test or generation command's exit code into a quality measurement. All 25 imported execution steps, including E2E, API/full-stack smoke, generation and deployment, retain their original command-hook definitions. PostgreSQL remains an execution service.

The `full` profile selects three collectors and nine policies (eight required, plus the unsupported Angular CRAP diagnostic). The `hook` profile explicitly declares partial assurance and omits expensive collection. The imported flow declares only `full` and `hook`; no `ci` profile is invented.

## Certified Rust and trusted baseline

The Rust binding preserves the accepted reference series `measurement-series/v1:4fa98f3128d8c3ac0dd5279ba9b4f4f9ae83cc3538575cfd5a6be1599a95dd98`, including native identity, rule, tool and runtime digests. Pack policies and capabilities are copied without modification. Thresholds, lineage compatibility, debt, ratchet and fail-closed evaluation retain their existing semantics; this configuration introduces no exceptions or waivers.

The required Git provider resolves the unique merge base with `origin/main`. The full workflow requires `.harness-gate/runtime/full-baseline-request.json`. The trusted host must independently authenticate the producer/run and baseline bundle, including base state and manifest digest, and supply matching configuration, source, measurement series and artifact provenance. Resolving a Git revision alone does not provide baseline measurements. Missing, stale, ambiguous or incompatible baseline inputs remain blocking under the existing provider contract.

Before runtime use, the Arc-Admin host must provision discovered canonical subjects and compiler metadata, actual state/artifact hashes, signed collector requests, trusted keys and the authenticated baseline request/bundle. Reference capability files are setup metadata, not measurements or evidence of an Arc-Admin quality PASS. A different native toolchain requires an explicit series/baseline migration, not replacement hashes to force compatibility.

See [quality compilation](../../../quality-compilation.md), [collectors](../../../quality-collectors.md), [trusted baselines](../../../quality-baselines.md), [ADR-0044](../../../adr/0044-trusted-quality-baselines.md) and [ADR-0040](../../../adr/0040-language-agnostic-evidence-policy.md).

## Validation boundary

[Validation evidence](validation.json) records the actual local results. The quality-binding regression validates this configuration with the imported flow and rejects broken relationships, series, required policy selection and baseline providers. Python regressions compare every composed pack and execution step to the accepted sources, including the certified Rust fixture and explicit Angular unsupported state. Existing certified collector/compiler/baseline acceptance exercises the underlying trusted quality path.

Those checks do not establish Arc-Admin source quality or runtime parity. Root `harness-gate config check` and `harness-gate verify --profile ci --all` are not applicable: this checkout has no `.harness-gate/flow.toml` and no declared `ci` profile. Hosted Required Quality Aggregate remains CI pending at submission.
