# Core follow-up reconciliation — Issue 34

Reviewed on 2026-10-15 against GateHouse main
`d8a328043dbd4776a7191d3263907b9bfc62599f`. This supplements the original
2026-10-14 review at `1c38fbbd8555e6a616ad29e83dea6cafe680e85a` and the
[Issue 15 receipt](../issue-15/README.md); it does not replace historical evidence.

Engineering Policy semantics remain unchanged. Rust Core retains requiredness,
thresholds, baseline/debt/ratchet, final decisions and release authority.
Measurement selections added by individual changes require fresh matching
base/head evidence; this reconciliation adopts no baseline or mixed series.

## Child delivery and evidence

All eleven child issues are closed. Each implementation PR below is merged;
its final check rollup has a successful Required Quality Aggregate. Workflow
jobs skipped by the existing classifier are not presented as executed tests.

| Issue | Accepted behavior | Merged delivery and evidence |
| --- | --- | --- |
| [#23](https://github.com/musutrade/GateHouse/issues/23) | Rename selection includes both components in working, staged and base modes; failure in the old component blocks verification | [PR #35](https://github.com/musutrade/GateHouse/pull/35), [66 regression scenarios and measurement receipt](../issue-23/README.md) |
| [#24](https://github.com/musutrade/GateHouse/issues/24) | Postgres checks, recipes and migration belong to GateProbes; required generic checks remain in Core | [PR #36](https://github.com/musutrade/GateHouse/pull/36), [release preparation #37](https://github.com/musutrade/GateHouse/pull/37), [public acceptance #38](https://github.com/musutrade/GateHouse/pull/38), [signed Core 0.5.0 / plugin 0.1.1 receipt](../../releases/postgres-service-0.1.1-acceptance.md) |
| [#25](https://github.com/musutrade/GateHouse/issues/25) | `unmatched=all` falls back only for actual unmatched changes | [PR #39](https://github.com/musutrade/GateHouse/pull/39); final issue receipt includes 64 policy/mode scenarios and 18 actual rename verifications shared with #23 |
| [#26](https://github.com/musutrade/GateHouse/issues/26) | No unconditional Rust probe; optional Git metadata is sampled once with timeout, output limits and cancellation | [PR #40](https://github.com/musutrade/GateHouse/pull/40); final issue receipt records metadata v2, 512 tests and three-platform hosted checks |
| [#27](https://github.com/musutrade/GateHouse/issues/27) | Runner v2 is declarative; Cargo argument/thread rules and migration are external data | [PR #41](https://github.com/musutrade/GateHouse/pull/41), merged [GateProbes #23](https://github.com/musutrade/GateProbes/pull/23), [contract and real Cargo acceptance](../../adr/0054-declarative-runner-inputs.md) |
| [#28](https://github.com/musutrade/GateHouse/issues/28) | Core installs only Core; legacy plugin flags diagnose before downloading; GateProbes owns plugin installation | [PR #42](https://github.com/musutrade/GateHouse/pull/42), merged [GateProbes #24](https://github.com/musutrade/GateProbes/pull/24), [public pinned installer and acceptance](../issue-28/README.md) |
| [#29](https://github.com/musutrade/GateHouse/issues/29) | Read-only `verify --plan` explains the same selection used by execution | [PR #43](https://github.com/musutrade/GateHouse/pull/43), [contract](../../verification-plan.md), [valid example](../issue-29/plan-example.json), [invalid example](../issue-29/plan-invalid-example.json) |
| [#30](https://github.com/musutrade/GateHouse/issues/30) | Explicit signed data-package pins and offline preflight share one generic interface | [PR #44](https://github.com/musutrade/GateHouse/pull/44), merged [GateProbes #25](https://github.com/musutrade/GateProbes/pull/25), [trust and compatibility contract](../../plugin-lock.md); PR records 29 real Rust/Python onboarding and lifecycle steps |
| [#31](https://github.com/musutrade/GateHouse/issues/31) | Explicit component impact graph selects transitive consumers and explains source chains | [PR #45](https://github.com/musutrade/GateHouse/pull/45), [graph validation, execution parity and retained first failure](../issue-31/README.md) |
| [#32](https://github.com/musutrade/GateHouse/issues/32) | Opt-in authenticated local step reuse binds declared inputs and rechecks current gates | [PR #46](https://github.com/musutrade/GateHouse/pull/46), [acceptance and both benchmark outcomes](../issue-32/README.md), [contract](../../step-cache.md) |
| [#33](https://github.com/musutrade/GateHouse/issues/33) | Generic findings, pinned history and SARIF; explicit advisory PR feedback in a separate action | [PR #47](https://github.com/musutrade/GateHouse/pull/47), [local acceptance](../issue-33/README.md), [contract and publication boundary](../../findings.md) |

The final implementation [CI run 38682199206](https://github.com/musutrade/GateHouse/actions/runs/38682199206)
passed all enabled checks on PR #47: Linux 574 tests with zero skips, macOS and
Windows tests, production coverage, fresh same-selection risk comparison,
17/17 mandatory Linux critical paths, Required Quality Aggregate and Generic
Quality Shadow. All 65 changed function identities were accepted with zero risk
failures. The PR records feature head `05bb67fa74188e1a2f6eb9e28017c6c1f42fea02`,
tested merge snapshot `67bb2fa0eb3e5693dd5b50b05d3ea494ad48dec6` and exact base
`dd429985066c43408cfbe676be5b84389794192d`. This is the accepted PR run, not
a claim that a later main run or a new release has completed.

## Ownership, compatibility and limits

- Core owns generic selection/planning, execution budgets, service lifecycle,
  check/result validation, trusted plugin preflight and decisions. GateProbes
  owns ecosystem measurements, Postgres semantics, Cargo runner definitions and
  plugin install/upgrade/rollback. Fixed external definitions change without
  rebuilding Core; ordinary Core CI consumes pinned products and does not build
  plugin implementation source. Existing [schema](../schema-ownership.md),
  [fixture](../fixture-ownership.md) and [release](../release-ownership.md)
  ownership inventories and source-absence checks remain in force.
- Postgres has actual independently verified signed public releases: Core
  `v0.5.0` and `postgres-service-v0.1.1`, including 47 live Linux contract records.
  Later source features in #25–#33 are merged delivery, not a claim that those
  features are present in published Core 0.5.0. Legacy flows require the explicit
  Postgres/runner migrations; rollback restores compatible configuration and
  binary together. Old tags, signatures, failures and evidence remain intact.
- Plugin locks cover explicitly prepared data-only preset packages. They do
  not grant adapter signing authority, install system tools, automatically adopt
  measurement series, discover plugins or provide a marketplace/service.
- Component graphs are reviewed project declarations with optional input pins;
  Core does not infer language imports or certify undeclared dependency truth.
- Step cache v1 is disabled by default and supported on Unix. Windows executes
  normally. Remote sharing, staged-root relocation and output restoration are
  outside v1. Recorded short workloads are 10.87% slower and longer workloads
  62.24% faster; neither synthetic series is a universal speed claim.
- Findings history remains diagnostic. GitHub-specific posting lives in
  `.github/actions/findings-feedback`, outside the Rust engine, and defaults
  off. Eleven offline API tests passed; no real PR annotations were published
  during acceptance. Fork publishing, live rendering certification, organization
  history services, dashboards and automatic fixes remain outside this delivery.
- Native RC8 remains a prerelease with legacy mutable assets. The pinned
  installer receipt does not claim a second publicly released version or fresh
  four-platform runtime certification. The paused bundled product stays retired.

## Umbrella closure

The eleven child deliveries, their independent cross-repository handoffs and
explicit first-stage limits satisfy Issue #34's scope. The append-only OpenSpec,
ownership and Postgres corrections link this current record without rewriting
their earlier receipts. This documentation reconciliation changes no runtime,
configuration/schema contract, gate, threshold, measurement source selection,
supported platform, baseline or release authority. New release work and broader
product capabilities require their own acceptance rather than being inferred
from closing the umbrella.
