# ADR 0051: Authenticated archive replay and private shared collector objects

Status: Proposed (GH-255)

## Context
The rc installer retains the full signed tar to recheck modes and archive identity. Compressed download caching does not eliminate per-version runtime and archive copies.

## Decision
Separate signed release authentication from materialized archive storage. A bounded receipt preserves tar headers and ordering; streaming installed bytes must reproduce the archive digest in the original signed inventory. Preserve SPDX, provenance, signatures and all license payloads. Keep private digest/mode-addressed shared regular files and serialize installation, migration and reference-aware cleanup. Never adopt arbitrary host tools by version alone or expose mutable external tool paths to capture.

## Consequences
Verification still reads all bytes but needs no full temporary archive. A receipt is a reconstruction aid, never a trust root. Shared-file mutation invalidates every affected version and must fail closed. Existing rc installs remain verifiable and can migrate after full verification. Release acceptance remains a publisher responsibility. Engineering Policy, measurement authentication and baseline authority are unchanged.
