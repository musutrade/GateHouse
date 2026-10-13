# Frozen Python product boundary

GH-146 implements OpenSpec task 1.1. This inventory assigns the production modules; the machine inventory is authoritative; it does not transfer runtime authority. A = CI/dev tooling, B = ecosystem adapter, C = generic product semantics, D = migration/reference tooling. Rows with `plugin_owner` are explicitly approved plugin-owned: their absence in a core-only tree is allowed per file, while core, frozen and oracle rows must exist and unregistered files must fail.

The [machine inventory](https://github.com/musutrade/Harness-Gate/blob/d9a932b23dcfea709175285b594ca200450a9971/docs/quality/gh-146/python-boundary.json) records direct callers, source hashes, purpose, CI role, release impact, current authority and final disposition for every module. Nested `tests/` and `fixtures/` programs are test assets, including the new D-class corpus replay helper. Source hashes pin the reviewed boundary; caller lists exclude documentation examples.

| Module | Class | Purpose |
| --- | --- | --- |
| `benchmarks.py` | A | Capture deterministic verification, test, and release-small baselines. |
| `ci_quality.py` | A | Required CI policy and fresh, review-only quality candidates (schema 1). |
| `collect_typescript_contracts.py` | B | Collect real task-3.3 contract runs in an isolated, disposable output directory. |
| `collector_runner.py` | C | Collector transport and validation boundary; never a release evaluator. |
| `complexity_analyzer.py` | B | Deterministic development complexity analyzer for the frozen fixture subset. |
| `contracts.py` | A | Exercise the public CLI contract and compare normalized golden snapshots. |
| `coverage.py` | A | Generate and gate source-boundary coverage evidence. |
| `critical_paths.py` | A | Fail-closed, isolated test-to-source critical-path evidence (rule v2). |
| `critical_paths_collect.py` | A | Collect fresh LLVM profiles from one exact nextest identity per path. |
| `cross_component.py` | C | Tool-independent contract bindings over already validated normalized evidence. |
| `docs_consistency.py` | A | Check Markdown links, embedded examples, and generated Schema sync. |
| `function_risk.py` | B | Versioned, source-location based joins of locked complexity and LLVM evidence. |
| `harness_evidence.py` | C | Standalone normalized evidence contracts; no release decision or collector. |
| `measurement_contract.py` | A | Capture the reproducibility contract for quality measurements. |
| `policy_engine.py` | C | Generic shadow policy evaluation over validated harness-evidence/v1 facts. |
| `policy_ratchet.py` | C | Baseline identity and review metadata for the generic shadow policy engine. |
| `post_remediation_benchmarks.py` | A | Capture R-16 scheduler/wait scenario baselines and R-17 validation evidence. |
| `production_coverage.py` | A | Strict production-location coverage; CI adoption is OpenSpec task 8.1. |
| `project_model.py` | C | Shadow project/subject contracts; no collector or release-policy authority. |
| `project_report.py` | C | Check opt-in project configuration and report generic shadow gates. |
| `quality_common.py` | A | Shared helpers for the repository quality evidence commands. |
| `quality_evidence.py` | C | Validate versioned quality evidence records (stdlib only). |
| `risk.py` | A | Build risk.json/risk.md from two retained, commit-bound measurement bundles. |
| `rust_equivalence.py` | D | Replay the pinned Rust acceptance window and its adversarial fixtures. |
| `rust_reference.py` | D | Project retained Rust candidate evidence and compare policies in shadow mode. |
| `source_measure.py` | B | Version 2 AST inventory, distinguishing closure instrumentation and LLVM join. |
| `typescript_acceptance.py` | D | GH-134: collect and replay two real, locked frontend history pairs. |
| `typescript_advisory.py` | D | Opt-in replay of the bounded frontend window; owns no required CI authority. |
| `typescript_contracts.py` | B | Development-only native contract adapter; tool parsing stays outside policy. |
| `typescript_reference.py` | B | Version 1 retained TypeScript/Angular collector; no policy decisions. |
| `typescript_semantics.py` | B | TypeScript identity/measurement primitives; not a collector or policy engine. |
| `core_isolation_check.py` | A | Manual isolation acceptance: build and test core with the plugin implementation sources removed. |

C-class final disposition is **frozen Python reference code after explicit authority-transfer acceptance**, with generic production callers routed through Rust. `collector_runner.py` retains Python transport; `quality_evidence.py` retains legacy Rust measurement validation while its shared generic schema semantics move. These mixed modules must not conceal generic authority behind adapter classification. A/B required-path helpers retain their existing authority and behavior. D tools remain advisory and can be retired only with reviewed replacement evidence.

The Rust-specific `risk.py` and production coverage thresholds remain required CI tooling. They are the current compatibility oracle, not the proposed generic Rust implementation. This distinction preserves ADR-0040 and the existing Required Quality Aggregate throughout tasks 1.1–1.2.
