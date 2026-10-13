# Implementation tasks

Parent: [proposal](proposal.md), [design](design.md), [delivery requirements](specs/rust-collector-delivery/spec.md).
Planning PR #226 and implementation P0–P8 are accepted. The
[current release and immutable handoff](../../../docs/release-status.md) records
production acceptance, publication, recovered readbacks and remaining GH-215 work.
Historical [GH-227 contracts](../../../docs/quality/gh-227/validation.md),
[GH-228 runtime](../../../docs/quality/gh-228/runtime.md),
[GH-229 lifecycle](../../../docs/quality/gh-229/validation.md) and
[GH-230 integration](../../../docs/quality/gh-230/acceptance.md) retain scoped evidence.
The original publication workflow failure is preserved; successful operator
recovery is separately identified. Completion is RC delivery, not stable promotion,
Core 0.4.1 native certification, baseline acceptance or Arc-Admin gate transfer.
Relevant Engineering Policy semantics remain unchanged.

## P0 Contracts

- [x] P0.1 [priority:P1] [effort:M, <=3h] Inventory existing protocol, native exits and authority boundaries; retain source references.
- [x] P0.2 [priority:P1] [effort:M, <=3h] Define typed measurement success/error and legacy-exit compatibility tests.

## P1 Identity

- [x] P1.1 [priority:P1] [effort:M, <=3h] Specify manifest fields and exact compatibility matrix; unknown combinations reject.
- [x] P1.2 [priority:P1] [effort:M, <=3h] Specify relocation negatives and reviewed series transition requirements.

## P2 Standalone runtime

- [x] P2.1 [priority:P1] [effort:M, <=3h] Inventory Python imports, frozen helpers and runtime/license closure.
- [x] P2.2 [priority:P1] [effort:M, <=3h] Implement private-runtime launcher and command contracts with no source-tree dependency.
- [x] P2.3 [priority:P1] [effort:M, <=3h] Separate measurement-only results from legacy policy-related exits; retain regression parity.

## P3 Build

- [x] P3.1 [priority:P1] [effort:M, <=3h] Pin compiler/runtime/dependency inputs and document verified host ABI.
- [x] P3.2 [priority:P1] [effort:M, <=3h] Build inventoried Linux bundle; compare two builds and document any nondeterminism.

## P4 Lifecycle

- [x] P4.1 [priority:P1] [effort:M, <=3h] Implement verified staging/atomic activation and interrupted-install tests.
- [x] P4.2 [priority:P1] [effort:M, <=3h] Implement isolated version selection, rollback and manifest-owned uninstall tests.

## P5 Supply chain

- [x] P5.1 [priority:P1] [effort:M, <=3h] Define protected independent-tag eligibility, SBOM and provenance subjects.
- [x] P5.2 [priority:P1] [effort:M, <=3h] Implement exact inventory verification and tamper/missing/extra asset rejection.

## P6 Integration

GH-230's [local acceptance](../../../docs/quality/gh-230/acceptance.md) supersedes
historical partial/blocker records and binds completion to fresh native captures,
installed generic invocation, released-Core decisions and independent producer counts.
The P6.1 subdivisions were authenticated request binding (<=3h), observed delivery
preflight/native re-export (<=3h), and installed generic invocation (<=3h).
P7 work was split into clean fixture/re-export, rejection/relocation probes, and
cost/archive accounting (each <=3h focused implementation); dependency waits and
failed environment attempts are retained separately. Subsequent production approval and P8 completion are recorded in the current
release handoff above.

- [x] P6.1 [priority:P1] [effort:M, <=3h] Bind installed collector through existing generic configuration and trusted requests.
- [x] P6.2 [priority:P1] [effort:M, <=3h] Test Core evaluation, unsupported capabilities and no duplicate producer/fallback launches.

## P7 Acceptance

- [x] P7.1 [priority:P1] [effort:M, <=3h] Run real clean-host fixture capture and full retained-binary re-export.
- [x] P7.2 [priority:P1] [effort:M, <=3h] Run relocation, malformed output, stale context, wrong tools and missing owner negatives.
- [x] P7.3 [priority:P1] [effort:M, <=3h] Measure cold/warm costs and durable artifact completeness; retain actual logs.

## P8 Handoff

- [x] P8.1 [priority:P1] [effort:M, <=3h] Run applicable local checks, strict OpenSpec, docs and required CI; retain results.
- [x] P8.1a [priority:P1] [effort:M, <=3h] Reconcile current-source documentation and local/hosted validation, retaining historical failures.
- [x] P8.1b [priority:P1] [effort:M, <=3h] Implement independently authenticated bootstrap and verify fresh-host positive and bootstrap negatives.
- [x] P8.1c [priority:P1] [effort:M, <=3h] Prepare immutable final candidate, dependency/license closure and durable exact asset receipts.
- [x] P8.1d [priority:P1] [effort:M, <=3h] Validate final candidate native/generic Core/lifecycle behavior and approve only evidenced compatibility entries.
- [x] P8.1e [priority:P1] [effort:M, <=3h] Prepare protected production workflow and dual RSA/Sigstore verification with rejection tests for review.
- [x] P8.1f [priority:P1] [effort:M, <=3h] Assemble publication packet and identify outstanding operator trust, environment and retention inputs.
- [x] P8.2 [priority:P1] [effort:M, <=3h] Obtain controller release approval and publish/verify exact RC assets.
- [x] P8.3 [priority:P1] [effort:M, <=3h] Deliver immutable RC receipt to Arc-Admin owner; record separate GH-215 acceptance/stable-promotion condition.

## Planning acceptance

- [x] [priority:P0] [effort:S, <1h] Strict OpenSpec validation and applicable repository documentation checks pass with retained commands/results. PR #226 exact-head validation and hosted documentation receipts are in design.md.
- [x] [priority:P0] [effort:S, <1h] Controller reviews planning PR; no automatic merge or symphony-ready label. PR #226 controller review and authorized merge are recorded in design.md; later execution authorization is separate.
