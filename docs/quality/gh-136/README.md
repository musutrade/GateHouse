# GH-136: TypeScript/Angular final acceptance review

Scope: task 4.3 of the [OpenSpec follow-up](../../../openspec/changes/typescript-angular-reference-adapter/tasks.md).
The review establishes a **bounded TypeScript/Angular reference adapter** for the
retained fixture, exact series and measured capabilities in the
[certification matrix](../typescript-certification.md). It does not establish
universal frontend certification. Final acceptance and issue closure remain
conditional on required CI passing on this PR's final SHA and controller merge.

## Predecessor acceptance

GitHub records checked on 2026-09-08 confirm all seven predecessor issues closed,
all implementation PRs merged, and each submitted head's Required Quality
Aggregate successful. Task 4.2's formerly pending hosted condition is satisfied
by PR #143. Its merge `67b0927f66be7768458ae4f1a433194925345e44` is this
workspace's starting revision. The [retained API snapshot](https://github.com/musutrade/Harness-Gate/blob/d9a932b23dcfea709175285b594ca200450a9971/docs/quality/gh-136/predecessors.json)
records full head/merge SHAs, completion times, individual check conclusions and
changed-file inventories. Skipped event-specific jobs remain recorded as skipped;
no skipped job is counted as passed. Historical milestone records saying
“pending at submission” describe their original submissions; this review supplies
their later acceptance evidence.

| Tasks | Closed issue / merged PR | Submitted head SHA | Required aggregate | Local/native evidence |
| --- | --- | --- | --- | --- |
| 1.1–1.2 | [#129](https://github.com/musutrade/Harness-Gate/issues/129) / [#137](https://github.com/musutrade/Harness-Gate/pull/137) | `98bb4c3cdca9e190ad111c0ab2b46e8fe8bd6a36` | [success](https://github.com/musutrade/Harness-Gate/actions/runs/34195289915/job/101963850568) | [retained evidence](../../../tools/quality/fixtures/typescript-angular/evidence/README.md) |
| 2.1–2.2 | [#130](https://github.com/musutrade/Harness-Gate/issues/130) / [#138](https://github.com/musutrade/Harness-Gate/pull/138) | `5db70cda1ccadefe87f7b04f2f25dff924d4598c` | [success](https://github.com/musutrade/Harness-Gate/actions/runs/34198098891/job/101972430518) | [retained evidence](../gh-130/README.md) |
| 3.1 | [#131](https://github.com/musutrade/Harness-Gate/issues/131) / [#139](https://github.com/musutrade/Harness-Gate/pull/139) | `6bc8e2e6e30eb7ea3b1b5c7ae476ce143b02132b` | [success](https://github.com/musutrade/Harness-Gate/actions/runs/34201259619/job/101983013393) | [retained evidence](../gh-131/README.md) |
| 3.2 | [#132](https://github.com/musutrade/Harness-Gate/issues/132) / [#140](https://github.com/musutrade/Harness-Gate/pull/140) | `6e694016fd80de427da4737feb852227404cea74` | [success](https://github.com/musutrade/Harness-Gate/actions/runs/34203538107/job/101989900180) | [retained evidence](../gh-132/README.md) |
| 3.3 | [#133](https://github.com/musutrade/Harness-Gate/issues/133) / [#141](https://github.com/musutrade/Harness-Gate/pull/141) | `2888033a3e843a0f4f4e5fc2cf161f4495bb3500` | [success](https://github.com/musutrade/Harness-Gate/actions/runs/34206490734/job/101999451376) | [retained evidence](../gh-133/README.md) |
| 4.1 | [#134](https://github.com/musutrade/Harness-Gate/issues/134) / [#142](https://github.com/musutrade/Harness-Gate/pull/142) | `bdceae294d9e0d2aee445fb7c81e54c63b9eea4d` | [success](https://github.com/musutrade/Harness-Gate/actions/runs/34209034166/job/102007838522) | [retained evidence](../gh-134/README.md) |
| 4.2 | [#135](https://github.com/musutrade/Harness-Gate/issues/135) / [#143](https://github.com/musutrade/Harness-Gate/pull/143) | `64bda19bb8569472493c08ce312e5db99d1868de` | [success](https://github.com/musutrade/Harness-Gate/actions/runs/34210938259/job/102014541640) | [retained evidence](../gh-135/README.md) |

The manual Frontend Advisory workflow is an opt-in retained replay. Its hosted
execution is not a native measurement platform acceptance claim. Local execution
is reproduced below; predecessor required CI is separate from that manual job.

## Certified boundary and architectural disposition

The [matrix](../typescript-certification.md#exact-fixture-and-toolchain) is the
exact certification boundary: four clean coverage revisions in two Git histories,
two independently retained quote-contract scenarios, pinned native tools and
Linux environment, and full coverage/contract/provider/consumer series descriptors.
The native archives, receipts, raw source/map/counter bytes and generated clients
remain committed under GH-133/GH-134. Replaying them neither recollects measurements
nor accepts a new baseline, runtime, platform or series.

Supported claims are original TypeScript file/function/method line and function
coverage with exact integer counters, unique source identity and validated maps,
the retained generic line-coverage threshold/ratchet/debt decisions, and the
bounded quote-contract compatibility/generated-client checks. Zero denominators
remain `not_applicable` without values. The series descriptor's metric list
does not override individual capability states.

TypeScript branch/region coverage, template/generated multi-source coverage,
route execution, browser E2E, SSR, accessibility, complexity, CRAP, mutation,
security, bundle size, performance and every other unmeasured capability remain
unavailable unless independently measured and accepted. Successful jsdom tests
and production builds do not certify Angular platform features or real browsers.
Repository Rust security checks do not certify frontend security.

TS-01–TS-04 each retain reproducer and evidence links in the
[mismatch table](../typescript-certification.md#architectural-mismatch-dispositions):
unique original-source identity; one source per subject with digested transformation
inputs; explicit requested capability states on every record; and fail-closed
changed-input/equal-generated-byte freshness. Reduced/ambiguous identity and
multi-source measurements remain outside support. No mismatch was hidden by
an adapter policy branch.

The predecessor changed-file inventories contain no generic core/protocol
amendment. This review also introduces none. A future generic amendment must
have its own reviewed OpenSpec delta plus Rust regression and real frontend
validation before adoption; it cannot be accepted through this adapter's scope.
[ADR-0040](../../adr/0040-language-agnostic-evidence-policy.md) continues to govern.

The [authority comparison](https://github.com/musutrade/Harness-Gate/blob/d9a932b23dcfea709175285b594ca200450a9971/docs/quality/gh-136/authority.json) verifies the required CI workflow,
release workflow, aggregate evaluator and frozen toolchain against the retained
GH-135 hashes. Required Quality Aggregate's identity, event-specific dependencies,
Rust defaults and required release authority stay unchanged. Migration requires
a future independent accepted change. Advisory rollback remains the
[documented disable/deselect operation](../typescript-certification.md#opt-in-operation-and-rollback),
preserving raw evidence, accepted baselines and debt.

## Validation and closure condition

[Validation results](https://github.com/musutrade/Harness-Gate/blob/d9a932b23dcfea709175285b594ca200450a9971/docs/quality/gh-136/validation.json), [replay summary](https://github.com/musutrade/Harness-Gate/blob/d9a932b23dcfea709175285b594ca200450a9971/docs/quality/gh-136/summary.json) and
[artifact digests](https://github.com/musutrade/Harness-Gate/blob/d9a932b23dcfea709175285b594ca200450a9971/docs/quality/gh-136/index.json) retain the final local commands and outcomes.
Complete command logs and the replay artifact inventory are in `validation.tar.gz`.
Cargo and Cargo subprocesses use `CARGO_TARGET_DIR=$PWD/target`, inside this
workspace. The replay checkout identifies the starting revision plus this
working-tree documentation review, not the final submitted PR SHA.

| Command | Actual local result |
| --- | --- |
| `python3 tools/quality/typescript_advisory.py --output target/quality/gh136/advisory` | Exit 0; 88 exact counter comparisons, zero unexplained mismatches; compatible coverage/contract pass and intentional coverage/contract fail; 22 acceptance and 11 contract tests pass |
| `cargo nextest run --manifest-path tools/harness-gate/Cargo.toml --locked` | Exit 0; 315 passed, 0 skipped |
| `cargo fmt --manifest-path tools/harness-gate/Cargo.toml -- --check` | Exit 0 |
| `cargo clippy --manifest-path tools/harness-gate/Cargo.toml --all-targets -- -D warnings` | Exit 0 |
| `python3 -m unittest discover -s tools/quality/tests -v` | Exit 0; 285 passed |
| `openspec validate typescript-angular-reference-adapter --strict` | Exit 0; change is valid |
| `python3 tools/quality/docs_consistency.py --output target/quality/docs-consistency.json` | Exit 0; status `pass` |

`harness-gate config check` and `harness-gate verify --profile ci --all` are
**not applicable**: `.harness-gate/flow.toml` is absent. No configuration was invented.
No new native frontend collection or manual hosted advisory execution is claimed.

The task 4.3 review is recorded; its final closure checkbox remains pending
required CI on this PR's final SHA. The controller owns that observation, merge
and issue closure. No whole-proposal completion, required-check migration or
final hosted CI success is asserted by this submission.

Delivery uses an isolated copy of this workspace’s Git metadata under ignored
`target/quality/gh136/delivery.git` because the supplied `.git` is read-only.
The copy preserves the prepared branch and baseline; it does not access another
workspace or the source checkout. The runtime handoff records the pushed SHA.
