# Architecture Decision Records (ADR)

This directory contains Architecture Decision Records (ADRs) for the Harness-Gate project.

## What is an ADR?

An Architecture Decision Record (ADR) is a document that captures an important architectural decision made along with its context and consequences.

## Format

We use a simplified version of the Michael Nygard ADR template:

- **Title**: Short noun phrase
- **Status**: Proposed, Accepted, Deprecated, or Superseded
- **Context**: What is the issue that we're seeing that is motivating this decision?
- **Decision**: What is the change that we're proposing and/or doing?
- **Consequences**: What becomes easier or more difficult to do because of this change?

## Index

- [ADR-0001](0001-use-rust-for-cli.md) - Use Rust for CLI implementation
- [ADR-0002](0002-optimize-release-builds.md) - Optimize release builds with LTO and strip
- [ADR-0003](0003-enhance-ci-pipeline.md) - Enhance CI pipeline with security and coverage checks
- [ADR-0004](0004-add-integration-tests.md) - Add integration tests for end-to-end scenarios
- [ADR-0005](0005-phase-2-optimization-strategy.md) - Phase 2 optimization strategy (performance and code quality)
- [ADR-0006](0006-test-fixtures-and-internal-boundaries.md) - Test fixtures and internal workflow boundaries
- [ADR-0007](0007-audit-module-decomposition.md) - Decompose the audit module by responsibility
- [ADR-0008](0008-config-module-decomposition.md) - Decompose the workflow configuration module by responsibility
- [ADR-0009](0009-secrets-module-decomposition.md) - Decompose the secret scanner by responsibility
- [ADR-0010](0010-verify-module-decomposition.md) - Decompose the verification module by responsibility
- [ADR-0011](0011-service-module-decomposition.md) - Decompose the service module by responsibility
- [ADR-0012](0012-process-module-decomposition.md) - Decompose the process module by responsibility
- [ADR-0013](0013-doctor-module-decomposition.md) - Decompose the doctor module by responsibility
- [ADR-0014](0014-preset-module-decomposition.md) - Decompose the preset module by responsibility
- [ADR-0015](0015-main-module-decomposition.md) - Decompose the CLI entry module by responsibility
- [ADR-0016](0016-project-module-decomposition.md) - Decompose the project module by responsibility
- [ADR-0017](0017-scope-module-decomposition.md) - Decompose the scope module by responsibility
- [ADR-0018](0018-audit-boundary-decomposition.md) - Complete the audit boundary decomposition
- [ADR-0019](0019-config-validation-decomposition.md) - Decompose configuration validation by responsibility
- [ADR-0020](0020-audit-scanner-decomposition.md) - Decompose the audit scanner by responsibility
- [ADR-0021](0021-refactor-documentation-closeout.md) - Close out refactor documentation after merge
- [ADR-0022](0022-terminal-feedback.md) - Provide terminal-aware CLI feedback
- [ADR-0023](0023-config-schema-and-interpolation.md) - Generate configuration schema and interpolate environment variables
- [ADR-0024](0024-verification-plan-dependencies.md) - Add dependency ordering to verification steps
- [ADR-0025](0025-phase-1-quality-baseline-gates.md) - Establish Phase 1 quality baseline gates
- [ADR-0026](0026-configuration-safety-diagnostics.md) - Add configuration safety diagnostics and future-concurrency preflight
- [ADR-0027](0027-unified-verification-plan.md) - Unify built-in gates and configured steps in a verification plan
- [ADR-0028](0028-parallel-scheduling.md) - Introduce dependency-aware parallel scheduling
- [ADR-0029](0029-project-scoped-configuration.md) - Keep workflow and audit configuration project-scoped
- [ADR-0030](0030-docs-rs-library-target.md) - Provide a documentation target for the CLI crate
- [ADR-0031](0031-harden-gate-boundaries.md) - Harden gate boundaries and delivery contracts
- [ADR-0032](0032-harness-gate-devrail-capability-contracts.md) - Define Harness-Gate capability contracts and the DevRail boundary
- [ADR-0033](0033-signed-out-of-process-adapter-protocol.md) - Define a signed out-of-process adapter protocol
- [ADR-0034](0034-fail-closed-trust-boundaries.md) - Enforce fail-closed trust boundaries
- [ADR-0035](0035-protect-release-eligibility.md) - Protect release eligibility and publication
- [ADR-0036](0036-verify-installer-artifacts.md) - Verify installer artifacts before atomic installation
- [ADR-0037](0037-adapter-request-integrity-and-evidence-budgets.md) - Bind adapter requests and bound evidence
- [ADR-0038](0038-post-remediation-hardening.md) - Plan post-remediation security, reliability, and maintenance follow-ups

- [ADR-0039](0039-required-risk-and-traceability-gates.md) - Require coverage, risk and traceability evidence in CI

- [ADR-0040](0040-language-agnostic-evidence-policy.md) - Keep language-agnostic evidence and policy behind a fail-closed migration boundary
- [ADR-0041](0041-quality-configuration-v1.md) - Define quality configuration v1 and explicit flow/quality validation

- [ADR-0043](0043-trusted-project-collectors.md) - Orchestrate configuration-driven project collectors through the signed adapter host
- [ADR-0044](0044-trusted-quality-baselines.md) - Resolve immutable baselines through ecosystem-opaque providers
- [ADR-0045](0045-quality-verification-composition.md) - Compose execution and generic quality in verify

- [ADR-0053](0053-stable-pure-rust-collector.md) - Require stable interfaces and pure-Rust collector delivery

## Creating New ADRs

When making a significant architectural decision:

1. Copy the template from an existing ADR
2. Number it sequentially (e.g., `0005-your-decision.md`)
3. Fill in the sections
4. Submit as part of your PR
5. Update this README index

## Superseding ADRs

When an ADR is superseded:
1. Update the status to "Superseded by ADR-XXXX"
2. Create the new ADR that supersedes it
3. Link between the two documents

- [ADR-0046: Capability-driven quality profiles and retained head evidence](0046-capability-driven-quality-profiles.md)

- [ADR-0047: Composable quality preset packs and explicit adoption](0047-composable-quality-preset-packs.md)

- [ADR-0048: Configured CI workflow acceptance](0048-configured-ci-workflow-acceptance.md)
- [ADR-0049: Keep project-owned validation outside Harness-Gate Core](0049-project-owned-validation-extension-boundary.md)
- [ADR-0050: Guaranteed serial dispatch and shared-service ordering](0050-serial-shared-service-ordering.md)

- [ADR-0051: Lightweight collector storage and independent upgrades](0051-lightweight-collector-storage.md)
