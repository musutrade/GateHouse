# Lightweight collector installation and independent upgrades

## Goals
Implement GH-255 T1–T7: demand-driven authenticated component delivery, shared runtime storage, independent plugin/Core/tool identities, archive-free verification, transactional upgrades, safe legacy migration and measured acceptance.

## Non-goals
No production publication, user installation changes, baseline adoption, weaker isolation, signatures, licensing or measurement policy. Engineering Policy remains unchanged. Only the observed Linux ABI is supported.

## Success Metrics
Exact downloaded and allocated bytes for compatible/clean/offline installations, three plugin upgrades, component upgrade and rollback; authenticated negative tests; real generic Rust compilation and Core evidence consumption. Missing native or production acceptance must remain explicitly incomplete.

## Risks
High: signed archive removal, shared-file mutation and crash recovery. Retain the original signed inventory and a bounded archive reconstruction recipe; verify the original signed archive digest by streaming installed bytes. Never trust an unsigned receipt alone. See [design](design.md), [tasks](tasks.md), [ADR 0051](../../../docs/adr/0051-lightweight-collector-storage.md).
