# Proposal: Dogfood Harness-Gate on Arc-Admin

## Summary

Use `musutrade/arc-admin` as the first real application dogfood target for the completed generic project-quality workflow, while making one durable architecture rule explicit: **project-owned validation remains project-owned**. Harness-Gate orchestrates, ingests structured results, collects normalized quality evidence, and decides required quality; it does not reimplement application-specific API/E2E/integration/smoke/load-test frameworks.

This change is intentionally migration-first and evidence-first. Arc-Admin's existing `cargo flow` system remains the initial assurance oracle. Harness-Gate is introduced in observation/shadow mode, compared for traditional-gate parity and added generic-quality value, and only then considered for authority transfer.

## Normative architecture delta

Add a durable Engineering Policy rule defining three extension layers:

1. **Command hooks / execution gates** — project-owned validation commands such as unit, integration, API, E2E, smoke, migration, generation, deployment and custom scripts. Harness-Gate owns orchestration semantics (scope/profile/dependencies/services/env/timeout/retry/requiredness/report linkage), not the test implementation.
2. **Structured result adapters** — optional ingestion of stable machine result formats such as JUnit, SARIF and declared JSON contracts, improving diagnostics without transferring policy authority.
3. **Quality collector plugins** — trusted measurement producers for facts such as coverage, complexity, CRAP inputs, bundle size or performance measurements. They emit normalized evidence; the released Rust core retains policy and final generic decision authority.

A new tool/framework SHALL NOT require built-in Harness-Gate awareness merely to participate as a command gate. Tool-specific built-ins are justified only when they implement a reusable protocol/format/certification boundary, not merely because a popular test runner exists.

## Why Arc-Admin

Arc-Admin is a production-style Angular 22 + Rust/Axum/SQLX + PostgreSQL 16 project with a mature project-owned workflow. Its current `cargo flow` config already orchestrates backend checks/tests, frontend lint/tests/build, Playwright E2E and full-stack smoke, OpenAPI/client generation consistency, template/framework checks, observability/deployment/supply-chain gates, services, scope routing and self-hosted CI.

That makes it a useful stress test for whether Harness-Gate can:

- import or migrate a substantial existing execution plane without manual re-entry;
- preserve every existing blocker while adding generic quality, CRAP, baseline/ratchet and one-project reporting;
- avoid duplicate execution and CI cost;
- keep project-specific test code and semantics in Arc-Admin;
- expose genuine Harness-Gate capability/UX gaps instead of hiding them by weakening Arc-Admin.

## Goals

- Freeze Arc-Admin's current required assurance inventory, scope semantics, services and CI behavior as the migration baseline.
- Define an import/migration path from structurally compatible `.arc-flow/flow.toml` content to `.harness-gate/flow.toml` without requiring users to manually rewrite dozens of steps.
- Add Harness-Gate `quality.toml` for Arc-Admin using the certified Angular + Rust + PostgreSQL model and existing project-specific command hooks.
- Run observation and shadow verification before any authority transfer.
- Compare scope selection, required traditional gate outcomes, diagnostics, report structure and wall/runner cost between `cargo flow` and Harness-Gate.
- Enable certified Rust coverage/CRAP/baseline/ratchet semantics without inventing Angular CRAP.
- Exercise controlled negative scenarios for command-gate failure, missing evidence, baseline mismatch and CRAP regression.
- Classify every failure as either Arc-Admin project debt/configuration or a Harness-Gate product/capability/UX gap.
- Require product gaps to be fixed in Harness-Gate rather than bypassed in Arc-Admin.
- Decide authority transfer only after parity, fail-closed behavior and cost evidence are accepted.

## Non-goals

- Do not move Arc-Admin's Playwright, API, full-stack smoke, OpenAPI generation, framework/template, deployment or supply-chain test logic into Harness-Gate.
- Do not delete or weaken existing Arc-Admin required gates to make dogfood pass.
- Do not make Harness-Gate understand Angular Material, Axum, SQLX, Playwright or PostgreSQL test semantics beyond generic configured execution/service/collector contracts.
- Do not certify Angular/TypeScript CRAP in this change.
- Do not add Go/Vue ecosystem support here.
- Do not remove Arc-Admin's existing `cargo flow` implementation until a separately evidenced authority-transfer step proves it is safe.

## Acceptance

The change is accepted only if:

- Engineering Policy permanently records the project-owned validation boundary and three extension layers;
- Arc-Admin's existing required gate inventory is captured and no blocker disappears during migration;
- compatible execution configuration is migrated/imported with minimal manual re-expression;
- Harness-Gate shadow results preserve traditional required-gate parity while adding valid generic quality decisions;
- project-specific API/E2E/integration/smoke logic remains project-owned command hooks;
- structured result ingestion, if used, improves diagnostics but does not become a competing policy engine;
- certified Rust CRAP/coverage/baseline/ratchet semantics work on Arc-Admin; Angular CRAP remains unsupported;
- controlled negative scenarios fail closed with actionable diagnostics;
- no duplicate authoritative measurement collection or unjustified duplicate command execution is introduced in CI;
- measured migration effort and CI cost are retained honestly;
- any Harness-Gate product gap is tracked/fixed before authority transfer rather than worked around by lowering Arc-Admin assurance.
