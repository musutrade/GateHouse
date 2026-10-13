# Proposal: Post-Remediation Security, Reliability, and Maintenance Hardening

**Status:** Implemented (2026-09-03)
**Date:** 2026-09-02
**Change type:** Security, platform reliability, machine contracts, performance, maintainability, and repository governance

## Scope Notice

This change records the contracts and acceptance evidence for R-07 and R-13
through R-18 from the remediation baseline. All tasks are implemented and
accepted; the umbrella does not modify DevRail traffic or staging authority,
and the deferred environment acceptance is tracked separately.

R-07's short-term documentation and reader-deadline work is already recorded
as complete elsewhere. This proposal preserves that wording and defines the
separate decision required before any operating-system sandbox claim.
Selected R-13 through R-18 implementation and focused tests are merged to
`main` as part of v0.3.6
([PR #75](https://github.com/musutrade/Harness-Gate/pull/75)); `tasks.md`
records the accepted evidence, including the R-07 wording inventory, OS
sandbox decision matrix, R-16/R-17 benchmark and allocation evidence, and the
8.x closeout. A future OS-sandbox implementation remains a separate non-goal;
DevRail staging acceptance remains external governance.

## Why

The current project has closed the original P0 trust-boundary findings and the
adapter request/evidence follow-ups, but the remaining audit findings still
matter before the executor is used with untrusted webhook configuration,
long-running cross-platform resources, or larger workflows. They also affect
whether downstream tools can safely consume errors and whether maintainers can
reason about configuration and release hygiene.

The findings have different implementation shapes, so the project needs one
shared contract with independently deliverable tracks rather than an implied
single completion flag.

## What Changes

- Records the bounded R-07 adapter-isolation wording and the separate decision
  boundary for a future OS sandbox.
- Defines the R-13 webhook host, address, DNS-rebinding, redirect, and redaction
  contract.
- Defines the R-14 cross-platform lease heartbeat and uncertain-ownership
  behavior.
- Defines the R-15 typed failure/diagnostic contract and the R-16 wait/scheduler
  performance contract.
- Defines the R-17 validation/configuration maintainability work and the R-18
  repository, community, and quality-policy work.
- Adds track-level acceptance evidence, migration constraints, benchmarks, and
  rollback requirements. Runtime changes for selected tracks are implemented
  incrementally on the associated branch and are not a production or staging
  acceptance claim.

## Goals

1. Keep adapter isolation claims limited to protocol and lifecycle controls
   until a separately approved OS-sandbox design exists.
2. Define a webhook egress policy that resists local-network access and DNS
   rebinding when configuration is not fully trusted.
3. Keep long-running leases attributable and alive for their entire resource
   lifecycle on Linux, macOS, and Windows, failing closed when identity is
   unavailable.
4. Establish stable typed failure and diagnostic contracts that do not depend
   on display-string parsing.
5. Reduce avoidable polling and scheduler scanning overhead while preserving
   ordering, cancellation, evidence, and failure semantics.
6. Make validation, enum, environment naming, panic-profile, repository, and
   contributor metadata behavior explicit and testable.

## Non-goals

- Do not implement an OS-level network, filesystem, resource, or complete
  process-tree sandbox in this change; that requires a separate ADR/OpenSpec.
- Do not treat incremental R-13 through R-18 implementation as production or
  staging acceptance; each change still requires the task evidence below.
- Do not change DevRail policy ownership, required-check ownership, staging,
  shadow/canary traffic, or rollback authority (G-03/G-04).
- Do not change the public result schema, configuration version, release
  artifact set, or installer trust chain without a track-specific migration
  decision.
- Do not mark a task or this change `implemented` without focused tests and
  reviewable evidence.

## Success Metrics

| Area | Success criterion |
| --- | --- |
| R-07 boundary | Documentation consistency checks contain no OS-sandbox or complete-descendant-isolation claim for the current adapter. |
| R-13 webhook egress | Private, loopback, link-local, unspecified, and rebinding resolutions are rejected; an explicit host allowlist is required and every connection decision is auditable without secrets. |
| R-14 lease liveness | A long step renews through execution and cleanup on supported platforms; uncertain identity never authorizes destructive cleanup. |
| R-15 typed diagnostics | Failure codes and retry classes serialize stably, configuration diagnostics carry structured fields, and no production decision parses display text. |
| R-16 performance | Wait/backoff and scheduler-index benchmarks show no semantic regressions and a reproducible improvement against the recorded baseline. |
| R-17 maintainability | Validation avoids whole-config cloning in hot paths, closed vocabularies reject unknown values, environment naming and panic-profile semantics are documented and tested. |
| R-18 repository governance | No tracked Python cache artifacts remain; security, MSRV, Issue/PR, and quality-script policies are present and checked by CI; secrets-scan wording matches actual behavior. |

## Impact and Risk

**Implementation risk: High.** Webhook egress and lease ownership alter trust
boundaries, while typed diagnostics and configuration changes can affect
machine consumers. The documentation-only change has no runtime risk, but it
must be precise enough that later implementation PRs cannot silently broaden
the contract.

| Dimension | Impact | Control |
| --- | --- | --- |
| Security | A webhook can be an outbound data path and DNS can change between resolution and connection. | Classify resolved addresses at connection time, require host policy, bound redirects or disable them, and add private/link-local/rebinding fixtures. |
| Platform | Process identity and lease APIs differ outside Linux. | Use a platform capability matrix; retain resources rather than guess ownership when a reliable identity is unavailable. |
| Compatibility | Typed codes, enum validation, and environment naming expose new machine-visible contracts. | Version serialized fields, preserve reviewed valid configurations, document migrations, and keep display text derived only from typed values. |
| Performance | Scheduler changes can alter ordering or cancellation if the ready set is wrong. | Preserve the existing dependency relation, benchmark before/after, and test cancellation, failure, and evidence paths. |
| Governance | Metadata and quality scripts can drift from the checks they describe. | Add documentation-consistency and CI checks for the policy files themselves. |

## Delivery Shape

The change is an umbrella contract, not one atomic implementation. R-13 through
R-18 can be delivered as separate PRs that reference this proposal and ADR-
0038. A track is complete only when its task-level acceptance evidence is
reviewed; the umbrella remains Proposed while any track is open.

## Dependencies and Assumptions

- The current R-06/R-07/R-09/R-11/R-12 implementation and protocol-v2
  evidence remain the compatibility baseline.
- DevRail remains the control plane for authorization, required checks, and
  rollout decisions.
- Cross-platform CI is available for behavior that can be tested on Linux,
  macOS, and Windows; platform-specific limitations are recorded when a
  fixture cannot run locally.
- Any new configuration key or serialized diagnostic field requires a separate
  compatibility review before implementation.

## Related Records

- [ADR-0038: Plan Post-Remediation Security, Reliability, and Maintenance Follow-ups](../../../docs/adr/0038-post-remediation-hardening.md)
- [Remediation baseline](../../../docs/code-remediation-2026-08-31-final.md)
- [Review follow-up status](../../../docs/review-followups-2026-08-31.md)
- [Adapter request integrity and evidence budgets](../adapter-request-integrity-and-evidence-budgets/proposal.md)
- [Harness-Gate and DevRail capability contracts](../harness-gate-devrail-capability-contracts/proposal.md)
