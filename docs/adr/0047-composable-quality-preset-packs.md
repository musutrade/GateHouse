# ADR-0047: Composable quality preset packs and explicit adoption

**Status:** Proposed for GH-185; hosted CI/controller acceptance pending.

**Date:** 2026-09-09

**Scope:** OpenSpec `integrate-generic-quality-into-project-workflow`, tasks 7.1–7.5.

## Decision

Reference presets consume declarative recipes, reusable ecosystem fragments and
an independent API contract capability pack. Packs own collector expectations,
measurement series, policy documents and profile participation. Generic preset
plumbing only expands bindings, merges data, checks output paths and writes files
atomically. The generic schema/compiler/verifier gains no ecosystem branches.

Rust defaults preserve the accepted reference coverage/CRAP limits, requiredness
and series in full/CI. TypeScript CRAP stays unsupported and optional. The mixed
preset composes those same packs and a relationship contract in one aggregate;
PostgreSQL stays an execution service. Hook has explicit partial assurance.

`generic` creates no quality configuration and preserves existing quality files.
Execution migration does not add policy. Adoption requires reviewed quality and
pack files, aligned execution identities/profiles and host-provisioned trusted
inputs. Initialization neither signs requests nor fabricates measurement evidence.
A missing runtime input fails closed. Baseline defaults to `none`; debt/ratchet
requires explicit compatible baseline and policy adoption.

## Consequences

These combinations are reference UX, not architecture. Future selections such as
`frontend=vue`, `backend=go`, `database=postgres` can compose execution, ecosystem
and capability pack data without bespoke pairwise core logic. This issue embeds a
small catalog and reference flow templates; registry discovery, installation,
version resolution, trust provisioning and full execution-pack composition are
future work. Pack capability metadata does not authorize a producer or grant PASS.
Actual installed toolchains must pin matching series and migrate baselines explicitly.

The synthetic unregistered ecosystem exercises the same composer and generic
cross-validation without a catalog entry. Regression guards reject ecosystem
branches and policy constants in generic preset plumbing. Other tests pin native
series/policies, unsupported boundaries, full/CI producer requirements, migration
opt-in behavior, output conflicts and every generated preset's config check.
See [preset and migration guidance](../quality-presets.md) and
[validation evidence](../quality/gh-185/validation.md).
