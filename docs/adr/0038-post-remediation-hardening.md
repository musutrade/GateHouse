# ADR-0038: Plan Post-Remediation Security, Reliability, and Maintenance Follow-ups

## Status

**Accepted** (2026-09-02; acceptance recorded 2026-09-03)

## Context

The 2026-08-31 remediation baseline is a historical audit record. R-01
through R-12 have since been implemented or, for R-07, narrowed to an explicit
short-term contract. The current implementation does not provide an
operating-system network, filesystem, resource, or complete process-tree
sandbox for adapters. R-13 through R-18 are the follow-up implementation
tracks recorded here:

- R-13: conditional webhook SSRF and egress policy;
- R-14: reliable non-Linux lease liveness for long steps;
- R-15: typed failure codes and structured configuration diagnostics;
- R-16: polling and scheduler performance;
- R-17: configuration and diagnostic maintainability; and
- R-18: repository hygiene, community files, and release metadata.

These concerns cross module and platform boundaries, but they have different
owners, risks, and acceptance evidence. Treating the whole list as complete
would turn a documented backlog into an unsupported security or reliability
claim. Treating it as one undifferentiated implementation would also make
rollback and review difficult.

The source findings and current status are recorded in
[`code-remediation-2026-08-31-final.md`](../code-remediation-2026-08-31-final.md)
and [`review-followups-2026-08-31.md`](../review-followups-2026-08-31.md).

## Decision

### 1. Preserve the R-07 short-term boundary

Harness-Gate documentation and adapter contracts SHALL describe capability
allowlists, process-group cleanup, bounded readers, and protocol checks as
protocol or lifecycle controls only. They SHALL NOT describe those controls as
an operating-system sandbox or complete descendant isolation.

An OS-enforced sandbox is explicitly deferred to a separate platform-specific
ADR and OpenSpec change. That future decision must identify the policy
primitive, supported platforms, identity and resource semantics, failure mode,
and cross-platform evidence before any stronger claim is published.

### 2. Track R-13 through R-18 as independent implementation tracks

The companion OpenSpec change, `post-remediation-hardening`, defines one
reviewable umbrella and six independently mergeable tracks. Each track must
have focused failure tests, a compatibility note, an owner, and a rollback
plan before its task is marked complete.

| Track | Required outcome | Priority boundary |
| --- | --- | --- |
| R-13 | Default-deny loopback, RFC1918, link-local, and other unspecified local targets; require an explicit host allowlist; re-check resolved addresses at connect time. | Conditional security control; required before accepting untrusted webhook configuration. |
| R-14 | Keep a lease alive for the complete resource lifecycle on every supported platform, using reliable platform identity; preserve resources when ownership is uncertain. | Reliability control for long-running cross-platform steps. |
| R-15 | Use a stable `FailureCode`, typed retry classification, and structured diagnostics; derive display text from those values rather than parsing prose. | Machine-contract and observability control. |
| R-16 | Replace fixed polling with a wait primitive or bounded backoff and reduce scheduler scans with indexed readiness/in-degree state, without changing execution semantics. | Performance follow-up; benchmark evidence is mandatory. |
| R-17 | Reduce validator cloning, make closed vocabularies typed, converge environment-variable naming, and document panic/unwind behavior with tests. | Maintainability and compatibility control. |
| R-18 | Complete repository hygiene and contributor/release metadata, and put quality scripts under the same quality policy they enforce. | Governance and maintenance control. |

### 3. Fail closed at the new boundaries

An invalid webhook destination, uncertain lease owner, unknown failure code,
ambiguous configuration vocabulary, or unverifiable performance/metadata
claim SHALL not be silently downgraded to the previous permissive behavior.
Where a platform cannot provide reliable identity or enforcement, the planned
behavior is to retain the resource or reject the operation and emit bounded,
structured evidence.

### 4. Stage implementation and acceptance

Implementation PRs may be delivered per track, but they share the OpenSpec
contract and this ADR. The release branch SHALL retain the current R-07
wording until a future OS-sandbox decision is accepted. No track may change
DevRail required-check ownership, production traffic, shadow/canary approval,
or rollback authority; those remain G-03/G-04 external governance work.

The ADR and OpenSpec remained `Proposed` until implementation evidence,
cross-platform checks where applicable, documentation consistency, and
rollback instructions were reviewed. Checked tasks link to that evidence and
must not imply that the deferred R-07 OS sandbox implementation is complete;
the bounded R-07 wording remains the compatibility contract.

## Consequences

### Positive

- The adapter isolation promise stays accurate while a real OS sandbox remains
  an explicit future decision.
- Security, lifecycle, typed-contract, performance, maintainability, and
  repository-governance work can be reviewed and rolled back independently.
- Each unresolved finding has a concrete acceptance boundary instead of an
  implied completion status.
- Cross-platform limitations become evidence requirements rather than hidden
  best-effort behavior.

### Negative

- The project carries several follow-up PRs and a longer acceptance sequence.
- A webhook policy may reject configurations that previously reached a network
  client, and a platform without reliable identity may retain resources until
  an operator resolves them.
- Stable failure codes and configuration enums become compatibility surfaces
  that require migration and documentation discipline.
- Performance work needs reproducible benchmarks and must not be justified by
  an unmeasured improvement.

## Alternatives Considered

- **Declare R-07 and R-13 through R-18 complete because the main branch is
  green:** rejected; CI success does not provide OS sandbox, private-network,
  non-Linux lease, typed-diagnostic, benchmark, or repository-metadata proof.
- **Implement all follow-ups in one large PR:** rejected; it couples security,
  platform lifecycle, performance, and maintenance changes and makes failure
  attribution and rollback harder.
- **Use a denylist of known private IP strings for webhooks:** rejected; DNS
  resolution and rebinding require address classification immediately before
  connection, plus an explicit host policy.
- **Infer failure and retry categories from human messages:** rejected; wording
  changes are not a stable machine contract and can misclassify recovery.
- **Advertise the existing process group as a sandbox:** rejected; cleanup and
  protocol allowlists do not enforce OS network, filesystem, resource, or full
  descendant isolation.

## Rollback

Each implementation track must provide a reviewed revert path and preserve
existing evidence. This ADR can be reverted without changing runtime behavior,
but a rollback must not re-enable
protocol-v1 requests, bypass webhook destination policy, remove a lease without
current ownership proof, or restore string-based machine decisions as an
undocumented compatibility shortcut.

## Related Records

- [ADR-0032: Define Harness-Gate Capability Contracts and the DevRail Boundary](0032-harness-gate-devrail-capability-contracts.md)
- [ADR-0037: Bind Adapter Requests and Bound Evidence](0037-adapter-request-integrity-and-evidence-budgets.md)
- [Remediation baseline](../code-remediation-2026-08-31-final.md)
- [Review follow-up status](../review-followups-2026-08-31.md)
- [Adapter isolation wording inventory](../adapter-isolation-wording-inventory.md)
- [OS sandbox decision matrix](../os-sandbox-decision-matrix.md)
- [OpenSpec: Post-remediation hardening](../../openspec/changes/post-remediation-hardening/proposal.md)

## Acceptance Evidence

This ADR is accepted for the R-07 wording boundary and the R-13 through R-18
implementation tracks. Selected implementation merged to `main` as part of
v0.3.6 ([PR #75](https://github.com/musutrade/Harness-Gate/pull/75)); PR CI
[33701746324](https://github.com/musutrade/Harness-Gate/actions/runs/33701746324)
and protected `main` CI
[33702793530](https://github.com/musutrade/Harness-Gate/actions/runs/33702793530)
passed, including the Linux/macOS/Windows test matrix and `Required Quality
Aggregate`. R-07 wording inventory and the OS-sandbox decision matrix are
recorded above; R-16/R-17 evidence and the 8.x closeout are recorded in the
companion OpenSpec. A future OS-sandbox implementation is explicitly outside
this accepted record, and DevRail staging G-03/G-04 acceptance is external
governance.
