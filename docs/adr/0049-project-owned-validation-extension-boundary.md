# ADR-0049: Keep project-owned validation outside Harness-Gate Core

## Status

Proposed

## Context

Harness-Gate now provides generic execution, trusted collector orchestration, baseline/ratchet handling and one authoritative Rust quality decision. As it begins dogfooding against mature applications such as Arc-Admin, there is a risk that project-specific validations—API tests, browser E2E, full-stack smoke, migration checks, load tests, generation checks or framework tooling—could be treated as missing Harness-Gate product features and reimplemented inside the product.

That would make Harness-Gate increasingly framework-specific, create permanent maintenance obligations for every popular test runner, and weaken the configuration-driven ecosystem model established by the generic quality architecture.

## Decision

Project-owned validation remains project-owned. Harness-Gate provides three generic extension layers:

1. **Command hooks / execution gates** for arbitrary project-owned validation commands. Harness-Gate owns orchestration and blocking composition, while the project owns test code and domain semantics.
2. **Structured result adapters** for reusable machine formats such as JUnit, SARIF and declared JSON contracts. These enrich diagnostics but do not acquire policy authority.
3. **Quality collector plugins** for normalized measurements that enter generic policy evaluation. Collectors measure; the released Rust core decides.

A new application test framework SHALL NOT require Generic Core changes merely to run through a command hook. Native support is justified only at a reusable protocol, result-format, service primitive, ecosystem/capability pack, certification or generic policy boundary.

When migrating a mature project, existing required validation is the assurance baseline until parity and fail-closed replacement behavior are proven. Unsupported migration semantics are Harness-Gate capability/UX gaps rather than permission to remove project gates.

## Consequences

Positive consequences:

- Harness-Gate remains ecosystem- and framework-neutral.
- Go/Vue, Java/React, Python/Svelte and unknown future stacks can keep their own API/E2E/integration tooling while using the same gate orchestration.
- Popular tool churn does not force Generic Core releases.
- Structured diagnostics and normalized quality evidence can evolve independently from application test implementations.
- Dogfood exposes genuine product gaps instead of incentivizing project-specific built-ins.

Costs and constraints:

- Some project validations will initially provide only command-level PASS/FAIL until a reusable structured-result adapter exists.
- Migration tooling must report unsupported execution semantics explicitly.
- A product feature request must distinguish reusable infrastructure from one project's custom validation logic.

## Alternatives rejected

### Build first-class integrations for every popular test framework

Rejected because it creates combinatorial product scope and couples the Core to ecosystem churn.

### Treat every test output as a quality collector

Rejected because command execution results and normalized quality measurements have different authority/trust semantics; forcing both through one model encourages fake metrics and confused policy ownership.

### Keep the boundary informal

Rejected because implementation agents can otherwise interpret dogfood failures as reasons to add framework-specific branches. The rule is therefore promoted to normative Engineering Policy and OpenSpec acceptance.

## Validation

GH-208's [authority-transfer recommendation](../dogfood/arc-admin/decision/README.md)
retains `cargo flow` authority and bounded shadow mode. Native quality, complete
host/routing parity and complete cost evidence remain blocked despite bounded
traditional command parity. Result remains validation evidence, not lifecycle
state. Existing infrastructure and assurance remain intact; this recommendation
does not change this ADR's proposed status or accept the whole dogfood proposal.

The [frozen Arc-Admin before state](../dogfood/arc-admin/README.md) records real Playwright E2E/full-stack smoke, Rust integration tests, OpenAPI generation consistency, PostgreSQL services and project-specific workflow gates. Its source hashes, inventory and historical CI timing calculations are checked offline by `docs/dogfood/arc-admin/reproduce.py` and the quality-script tests.

`project_owned_runner_replacement_preserves_generic_command_gate` in `tools/harness-gate/tests/failure_paths_test.rs` exercises two unknown executable runners through the same generic configuration contract, including blocking failures, logs and sealed evidence. Documentation consistency requires the extension-layer and project-ownership policy anchors. No Generic Core or schema special case is added.

GH-202 adds [execution import fixtures, loss detection and UX measurements](../dogfood/arc-admin/import/README.md) for all 25 baseline blockers. E2E/API/smoke/generation/deployment commands remain unchanged project declarations. Import requires an explicit execution-only scope and reports runtime incompatibilities; neither these fixtures nor the baseline establish shadow parity or authority transfer. The complete OpenSpec change and this ADR remain proposed.
