# Split Doctor Module

This change decomposes doctor reporting, check dispatch, check helpers, and
tests while preserving the existing `crate::doctor` boundary and CLI output.

- ADR: [0013-doctor-module-decomposition](../../../docs/adr/0013-doctor-module-decomposition.md)
- Status: merged to `main`; `main` CI passed
