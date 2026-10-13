# Split Process Module

This change decomposes signal handling, command capture, task execution, and
process lifecycle helpers while preserving the existing `crate::process`
boundary and behavior.

- ADR: [0012-process-module-decomposition](../../../docs/adr/0012-process-module-decomposition.md)
- Status: merged to `main`; `main` CI passed
