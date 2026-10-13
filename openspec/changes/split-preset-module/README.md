# Split Preset Module

This change decomposes preset catalog data, initialization, migration,
filesystem helpers, and tests while preserving the existing `crate::preset`
boundary and CLI behavior.

- ADR: [0014-preset-module-decomposition](../../../docs/adr/0014-preset-module-decomposition.md)
- Status: merged to `main`; `main` CI passed
