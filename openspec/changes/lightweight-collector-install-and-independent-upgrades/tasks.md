# GH-255 authorized tasks

Each subtask is scoped below four hours. Check only with retained evidence in `docs/quality/collector-install-costs-20260913.md`.

- [x] T1 [P0, S] Inventory archive/verification dependencies; proposal, design, spec and ADR aligned; strict OpenSpec validation.
- [x] T2 [P0, M] Exact component/Core/protocol/ABI/measurement compatibility and explanations; negative compatibility tests.
- [x] T3 [P0, M] Default missing-object acquisition, byte plan, CI policy, pinned/offline modes and automatic trust verification.
- [x] T4 [P0, M] Archive-free signed replay verification, shared storage, bounded cache and explicit verified export.
- [x] T5 [P0, M] Independent upgrades, self-check before activation, rollback and failure recovery; identity protection unchanged.
- [x] T6 [P0, M] Usage/cleanup, retained-version references and recoverable rc migration; exact reclaimed bytes.
- [x] T7 [P0, M] Release integration, help/docs and repository checks; real native and size/upgrade acceptance with limitations.

## Validation and acceptance boundary

The checked tasks record implementation and the local validation described in the linked cost report: 397 Core tests, 415 quality tests (three environment-dependent groups separately exercised: 12 private-runtime and 17 native-driver tests), 83 release tests, shell integrity tests, native offline installs, three incremental plugin upgrades, migration/export and rollback. Formatting, Clippy, documentation consistency and strict OpenSpec validation pass.

This proposal is **not declared fully accepted or released**. Production dual-signature online/clean-host acceptance, a second real compiler upgrade, and continuous total temporary peak measurement remain release-runner acceptance gaps. Local temporary RSA eligibility and synthetic fault/component tests do not substitute for those results. T7 integrates the protected publisher and documents these limits; it does not authorize a production publication or baseline adoption.
