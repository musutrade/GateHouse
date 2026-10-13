# Design: Real TypeScript/Angular abstraction validation

## Fixture and measurement decisions

GH-129 implements tasks 1.1–1.2 with the CLI-generated repository fixture at
`tools/quality/fixtures/typescript-angular/app/` and its small Rust provider.
The [frozen toolchain](../../../tools/quality/fixtures/typescript-angular/toolchain.json)
selects Angular/CLI/build 22.0.8, Node 24.18.0, npm 11.16.0, TypeScript 6.0.2,
Vitest/Istanbul 4.0.8 and openapi-typescript-codegen 0.29.0. Exact package versions
and lockfiles are committed. The [native evidence](../../../tools/quality/fixtures/typescript-angular/evidence/README.md)
retains repeated clean-install inventories, successful real build/tests,
configuration digests, source bytes, raw counters and production/test source maps.
Compatibility is demonstrated by these operations, not inferred from the
synthetic frontend fixture. GH-130 tasks 2.1–2.2 add the [identity and measurement semantics](../../../docs/quality/typescript-source-semantics.md).
TS-01/TS-02 are resolved within that bounded scope. GH-131 task 3.1 adds the
[retained collector](../../../docs/quality/typescript-collector.md) and resolves
TS-03 without a generic contract change. GH-132 task 3.2 adds [generic policy validation](../../../docs/quality/gh-132/README.md)
using native retained replay and explicitly controlled policy derivations alongside
the Rust reference gates. GH-133 task 3.3 adds [real contract integration](../../../docs/quality/gh-133/README.md):
pinned oasdiff output and freshly generated client bytes enter unchanged generic
cross-component gates. Passing native local gates coexist with a blocking real
OpenAPI change. GH-134 task 4.1 adds [real acceptance history pairs](../../../docs/quality/gh-134/README.md):
four fresh native runs retain clean Git revisions, exact counters and matching
generic outcomes/debt for a compatible change and a real coverage regression.
The negative matrix reproduces integrity, identity, capability, series and
collection failures. No new architecture mismatch or generic contract delta is
introduced. GH-135 task 4.2 adds [opt-in advisory replay and local validation](../../../docs/quality/gh-135/README.md) and the [bounded certification matrix](../../../docs/quality/typescript-certification.md), including reproducer/evidence links for TS-01–TS-04 and rollback. [GH-136 final review](../../../docs/quality/gh-136/README.md) confirms merged PRs and successful required CI for tasks 1.1–4.2, unchanged TS-01–TS-04 boundaries and no generic amendment. Final closure remains conditional on required CI for the review PR and controller acceptance.

The application will have a component with an external template, a service with
tested and untested branches, same-named methods in distinct classes, a lazy
route, and a generated client for a small Rust provider's OpenAPI contract.
Use real build/test/coverage and contract-tool output. Retain argv, tool versions,
exit status, base/head/run/target, source bytes, raw counters, source maps,
generated-client artifacts and their digests. A build failure or partial collection
cannot supply a passing evidence batch.

Angular documents configurable coverage reporters and coverage collection via
`ng test --coverage`; actual commands must be confirmed against the fixture's
pinned builder. See [Angular coverage](https://angular.dev/guide/testing/code-coverage).
TypeScript source maps relate emitted JavaScript to original sources; preserve
and validate that relationship, including source contents, rather than joining
by basename. See [TypeScript sourceMap](https://www.typescriptlang.org/tsconfig/sourceMap.html).

## Existing interfaces and authority

Use `harness-collector-request/v1` and `harness-collector-response/v1` through
the GH-113 runner, with `harness-project/v1` subjects and `harness-evidence/v1`.
Configure the mixed project through the opt-in JSON project manifest; the
conceptual TOML in the architecture design is not supported syntax.

The adapter owns source parsing, coordinate normalization, raw-tool formats and
series definition. Generic policy owns thresholds, support requirements,
ratchets and project aggregation. No `language == "typescript"` branch belongs
in the policy engine. Tool exit facts remain distinct from policy outcomes.

Initial acceptance targets source-backed file/function/method line and function
coverage with covered/total counters. Branch coverage is claimed only for the
mapped TypeScript constructs actually measured. Complexity and CRAP remain
`unsupported` until their own compatible measurement contract is implemented;
never substitute file coverage for function coverage. A zero denominator is
explicitly unavailable, not perfect coverage. Requested unavailable capabilities
retain one of the existing six states and cannot acquire numeric defaults.

Series identity includes compiler, builder, test runner, coverage provider,
mapping/rule versions, runtime/target, boundary and normalization semantics.
Changed semantics create a new series requiring explicit baseline acceptance.
Rust and TypeScript series are never compared as interchangeable. Use explicit
modify/rename/move lineage for digest-changing subjects and retain historical debt.

## Architectural mismatch register

These findings were recorded while drafting GH-119, before running a real
TypeScript adapter. They limit acceptance; they are not proof of tool behavior.

| ID | Evidence / mismatch | Required disposition and owner |
| --- | --- | --- |
| TS-01 | Architecture design §2 describes reduced identity negotiation; executable v1 requires a nonempty discriminator and source digest. [Project model](../../../docs/quality/project-model.md) explicitly excludes reduced identity negotiation. | Identity task 2.1 must reject ambiguous/anonymous mappings unless it can derive an unambiguous versioned source identity. If reduced identity is needed, propose a reviewed generic schema/protocol delta; never forge a name or digest. **Resolved by GH-130 / task 2.1:** pinned parser ownership/kind/span and original bytes satisfy full v1 identity; exact declaration joins reject ambiguity. Reduced identity remains unsupported. [Decision and evidence](../../../docs/quality/typescript-source-semantics.md#ts-01-original-source-identity). |
| TS-02 | A v1 subject has one source path/digest and optional span, while external templates, emitted JavaScript and source maps involve multiple files. Their joint identity is not a defined v1 subject contract. | Tasks 2.1–2.2 must bind source-backed subjects to original files and retain transformation inputs as digested artifacts. Certify TypeScript only initially. Template/generated multi-source coverage remains unsupported unless real fixtures establish an adequate representation or a reviewed core delta. **Resolved by GH-130 / tasks 2.1–2.2:** one original TypeScript source per subject, caller-pinned artifact provenance and validated test mappings. Template/generated multi-source measurements remain unsupported; no generic amendment requested. [Decision and evidence](../../../docs/quality/typescript-source-semantics.md#ts-02-transformation-provenance-and-boundaries). |
| TS-03 | The collector runner requires every requested capability on each returned record; requests cannot currently express separate capability sets per subject kind. See [collector contract](../../../docs/quality/collector-protocol.md). | **Resolved by GH-131 / task 3.1:** mixed file/function/method/route replay declares every requested generic capability on every record. Routes and unmeasured capabilities are explicitly unsupported; empty native denominators are not applicable. Scoped capability objects and silent omission are rejected by the existing runner. [Evidence and limits](../../../docs/quality/gh-131/README.md); no generic request-scope delta or language-specific policy branch is needed. |

TS-04 concerns generated byte drift versus the existing input-digest freshness
invariant. GH-133 retains an equal-byte/changed-input reproducer and fails closed
when they disagree; capability is narrowed to the evidenced fixture cases without
a generic amendment. See the [disposition and tests](../../../docs/quality/gh-133/README.md#fail-closed-behavior-and-bounded-capability).

For each finding, retain the reproducer, expected and actual generic behavior,
affected contract, decision, PR and validation evidence. Adapter-specific parsing
is appropriate; altering generic semantics inside an adapter is not. An unresolved
finding may be deferred only by narrowing the documented capability scope;
it cannot be counted as a passed second-ecosystem capability.

## Acceptance, rollout and rollback

Run locked install, Angular build, tests and coverage once per fixture revision.
Normalize retained output, replay generic policy and ratchets, and compare exact
native counters for two base/head pairs. Include intentional missing coverage,
duplicate symbols, stale/tampered source maps, changed tool series, unsupported
metrics, failed subprocesses and debt-preservation cases.

A real OpenAPI compatibility check and generated-client drift check must exercise
the existing provider/consumer relationship and project report: local component
gates may pass while the contract gate fails. If tools cannot support a claimed
metric, return its explicit unavailable state and block that acceptance criterion.

Add an opt-in advisory job with retained artifacts and a certification matrix
limited to the pinned fixture, environment, series and capabilities. Keep Rust's
required aggregate untouched. Rollback disables this job/adapter selection and
retains evidence and accepted baselines; no authoritative gate needs replacement.
Generic changes, if required, need both Rust regression and real frontend evidence.

## Alternatives and implementation sequence

Synthetic JSON alone cannot validate frontend mapping, so it remains a unit-test
aid. Treating emitted JavaScript coverage as TypeScript coverage would hide source
identity errors. A new frontend-specific policy engine would bypass the abstraction.

Execute [tasks](tasks.md) in order: freeze tools and fixture (two M tasks), define
identity/series (two M tasks), implement and integrate (three M/L tasks), then run
acceptance and document limits (three M/S tasks). Each task is under four hours;
split any overrun before implementation. No calendar delivery date is asserted.
