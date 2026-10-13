# Tasks: Configuration Safety Diagnostics and Future-Concurrency Preflight

**Parent:** [proposal.md](proposal.md), [design.md](design.md), and
[configuration-safety specification](specs/configuration-safety/spec.md)
**Status:** Implemented with one documented renderer follow-up; compatibility
evidence is linked to PR #34 and the subsequent project-scoping fix in PR #38.
**Implementation restriction:** This change authorizes configuration safety
infrastructure only; it does not introduce parallel scheduling or template
rendering.

Every eventual task is intentionally bounded to less than four hours. A task
may be checked only when its acceptance evidence is reviewed and all
compatibility requirements in the parent design remain true.

## 1. Contract and Diagnostic Inventory

- [x] **1.1 (P0, S)** Approve the diagnostic-ID registry and canonical field
  path grammar.
  **Acceptance:** The registry covers every error class in the specification;
  path escaping/indexing examples are snapshot-approved and no ID is inferred
  from human wording.

- [x] **1.2 (P0, S)** Define the version-1 JSON diagnostic envelope and the
  human-rendering compatibility matrix.
  **Acceptance:** Valid/invalid/truncated examples parse against a reviewed
  JSON schema or equivalent contract; stdout/stderr/exit outcomes are defined
  for human and JSON modes.

- [x] **1.3 (P0, S)** Inventory current parse, interpolation, semantic,
  dependency, service, and log failure messages and their snapshots.
  **Acceptance:** Each existing behavior is mapped to a planned stable ID,
  primary path, help text, and compatibility disposition; uncertainties are
  listed explicitly rather than guessed.

- [x] **1.4 (P1, S)** Define source-map coverage and degradation rules for file,
  in-memory, TOML parse, and interpolation inputs.
  **Acceptance:** The approved matrix states exactly when location/range can be
  absent and proves that path/ID/help remain present.

## 2. Static Resource-Safety Contract

- [x] **2.1 (P0, S)** Define the deterministic dependency reachability and
  potential-concurrency test matrix.
  **Acceptance:** The matrix covers direct order, transitive order, inverse
  order, unrelated steps, profile variation, and all-scope selection; expected
  concurrency truth values are reviewable.

- [x] **2.2 (P0, S)** Define injection-collision precedence and related-path
  rules.
  **Acceptance:** Same-service and distinct-service cases each emit the exact
  planned diagnostic set without duplicate/conflicting messages.

- [x] **2.3 (P0, S)** Define normalized log identity and duplicate-log
  compatibility policy.
  **Acceptance:** Existing filename restrictions, separator/case assumptions,
  and ordered/unordered duplicate outcomes are recorded for Linux, macOS, and
  Windows.

- [x] **2.4 (P1, S)** Specify the future scheduler handoff and runtime-lock
  invariants.
  **Acceptance:** The handoff identifies inputs/outputs and confirms that the
  scheduler cannot relax rejected static relations without a new ADR/OpenSpec
  decision.

## 3. Future Template-Safety Specification

- [x] **3.1 (P0, S)** Approve the lexical, canonical, and symlink-containment
  path matrix for future template roots and files.
  **Acceptance:** The matrix includes relative valid paths, absolute paths,
  `..`, platform prefixes, NUL, missing targets, directories, and symlink
  escapes, with platform applicability recorded.

- [x] **3.2 (P0, S)** Define report-root/template-root separation and confined
  include/inheritance/asset-loader behavior.
  **Acceptance:** Equal, ancestor, descendant, and unrelated roots have
  explicit decisions; no case permits renderer writes beside template input.

- [x] **3.3 (P1, S)** Review the future schema and migration impact before
  adding renderer-specific report-template configuration.
  **Acceptance:** A separate renderer proposal identifies rendering mode,
  defaults, schema diff, compatibility policy, include/inheritance loader
  confinement, and validation tests; the existing input-path fields are not
  widened implicitly.

## 4. Future Implementation and Test Evidence

- [x] **4.1 (P0, M)** Implement typed diagnostics, source mapping, aggregation,
  and human rendering at the loader/validator boundary.
  **Acceptance:** Representative parser, interpolation, field, reference, and
  multi-field errors carry the specified ID/path/help/location semantics, are
  deterministic, and leak no resolved secret values.

- [x] **4.2 (P0, S)** Implement the explicit `config check --format json`
  renderer without altering default human output.
  **Acceptance:** Valid, invalid, and truncated JSON envelopes match version 1;
  stdout is JSON-only, stderr has no diagnostic prose, and the established
  status/error-category behavior is preserved.

- [x] **4.3 (P0, M)** Implement dependency reachability and static service/log
  resource preflight.
  **Acceptance:** Ordered reuse passes; unordered distinct-service injection,
  unordered shared service, and duplicate logs fail before any external work;
  primary/related paths and repair help match the specification.

- [x] **4.4 (P0, M)** Implement report-template path validation without adding
  rendering.
  **Acceptance:** Lexical/canonical/symlink containment, regular-file checks,
  and root separation pass on each applicable platform; include/inheritance
  loader confinement remains a prerequisite of the separate renderer change;
  existing report outputs remain unchanged.

- [x] **4.5 (P0, M)** Add unit, integration, and CLI-contract tests for all
  diagnostic and resource-preflight scenarios.
  **Acceptance:** Tests cover source locations, deterministic ordering,
  truncation, secret redaction, direct/transitive/unrelated dependencies,
  duplicate logs, and platform path cases; ADR-0025 snapshots are reviewed.

## 5. Documentation, Migration, and Closeout

- [x] **5.1 (P1, S)** Add verified VS Code Even Better TOML and Taplo local
  schema-association guidance.
  **Acceptance:** Documentation uses the committed local schema, distinguishes
  structural/schema validation from `config check`, and passes link/example
  validation.

- [x] **5.2 (P1, S)** Add migration guidance and valid/negative configuration
  fixtures.
  **Acceptance:** v1 migration, existing-valid-v2, each resource conflict,
  missing interpolation, and template containment cases have linked and
  executable documentation evidence.

- [x] **5.3 (P0, S)** Extend the ADR-0025 documentation-consistency inventory
  for new guidance and fixtures.
  **Acceptance:** Schema regeneration/diff, example loading, migration checks,
  links/fragments, and negative expected failures run in the documented local
  and CI commands.

- [x] **5.4 (P0, S)** Run compatibility and cross-platform verification before
  acceptance.
  **Acceptance:** Formatter, linter, tests, configuration snapshots, and
  Linux/macOS/Windows structured checks are green; known platform limitations
  have an owner and expiry.

- [x] **5.5 (P0, S)** Update ADR/OpenSpec status only after implementation and
  evidence review.
  **Acceptance:** ADR-0026, this change, quality artifacts, implementation PR,
  and any renderer follow-up decision are linked; no incomplete task is marked
  complete.

## Evidence Review

- Implementation: [PR #34](https://github.com/musutrade/Harness-Gate/pull/34)
- Project-scoped configuration compatibility fix: [PR #38](https://github.com/musutrade/Harness-Gate/pull/38)
- Closeout and green CI: [PR #39](https://github.com/musutrade/Harness-Gate/pull/39)
- Cross-platform quality evidence: [run 33217867592](https://github.com/musutrade/Harness-Gate/actions/runs/33217867592)
- Renderer follow-up: [confined report-template rendering proposal](../report-template-renderer/proposal.md)
