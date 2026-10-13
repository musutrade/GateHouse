# Split Service Module

This change decomposes service management, PostgreSQL validation, Docker
lifecycle, and tests while preserving the existing `crate::service` boundary.

- ADR: [0011-service-module-decomposition](../../../docs/adr/0011-service-module-decomposition.md)
- Status: merged to `main`; `main` CI passed
