# Priority Follow-up Status (2026-09-01)

This is the current status after the v0.3.5 release. The historical review
baseline remains unchanged in
[`review-followups-2026-08-31.md`](review-followups-2026-08-31.md).

## Completed In This Change

- **R-06:** Adapter protocol v2 signs the complete canonical request and binds
  invocation, step, configuration, arguments, input, environment, capabilities,
  timeout, artifact root, nonce, and validity window. Expiry, tampering,
  invalid environment entries, and nonce replay fail before execution.
- **R-07 (short-term contract):** README, configuration docs, ADR, and OpenSpec
  now distinguish protocol allowlists/process-group cleanup from an OS sandbox.
  Reader completion has an independent deadline. Platform-enforced sandboxing
  remains a separate future design.
- **R-09:** Audit JSON/Markdown, parsed-log structured/fallback output,
  verification evidence, webhooks, audit errors, and release SBOM source
  references share credential-safe redaction boundaries.
- **R-11:** Adapter, task, and generic process output use bounded readers;
  adapter artifacts and requests have size budgets; invocation evidence is
  bounded to 256 MiB with a 16 MiB per-file limit; overflow and reader
  deadline outcomes are structured failures with partial-byte/truncation
  evidence.
- **R-12:** JUnit/TRX ingestion now requires one recognized root, rejects
  missing or multiple roots and non-whitespace trailing content, and retains
  namespace-prefixed standard result compatibility.

## Still External

- **G-03/G-04:** A real DevRail staging environment, an approved shadow/canary
  slice, and rollback authority are still required. The local DevRail checkout
  was inspected read-only: its `.arc-flow/flow.toml` passes schema-v2
  `config check`, and `scope --all --json` returns `backend`, `frontend`, and
  `workflow`. It has unrelated uncommitted work, while the local service
  readiness endpoints return `401 Unauthorized`; no usable authenticated
  staging session is available in this workspace. Therefore no
  production-like mapping or traffic switch is claimed here.
- **D-02:** The DevRail ADR/OpenSpec remains pending until G-03/G-04 evidence is
  attached. DevRail retains policy, required-check ownership, and release
  decisions.
- **D-06:** The report-template renderer proposal remains intentionally
  Proposed pending a product decision on schema, defaults, compatibility, and
  sandbox requirements.

## Verification

The pull request for this change records focused and full Rust tests, formatter,
Clippy, OpenSpec validation, and documentation consistency results. The v0.3.5
release evidence is recorded separately in the release closeout ADR/OpenSpec.
